//! audio.cpp CLI process management: path resolution, spawn, line callbacks,
//! stderr ring buffer, exit-code mapping.
//!
//! Task: P1-6. See docs/tech-spec/refactor-audiocpp.md §5.1.
//!
//! Path resolution must reuse the ffmpeg rule in `commands::video`
//! (dev: `src-tauri/resources/bin/`; prod: `Resources/resources/bin/`).
//!
//! Pinned baseline — keep these signatures stable for P1-8/P1-9/P1-15.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::process::Stdio;

use tokio::io::{AsyncBufReadExt, BufReader};

/// Where a `--backend` value came from; P1-9 retries `metal` failures on `cpu`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Backend {
    Metal,
    Cpu,
}

/// A finished or running CLI invocation request.
#[derive(Debug, Clone)]
pub struct CliInvocation {
    pub task: String,
    pub family: String,
    pub model: PathBuf,
    pub audio: Option<PathBuf>,
    pub backend: Backend,
    pub args: Vec<String>,
}

/// Captured result of a completed CLI run.
#[derive(Debug, Clone)]
pub struct CliOutput {
    pub status: std::process::ExitStatus,
    pub stdout: String,
    /// Last 50 stderr lines, joined with `\n` (spec §5.1).
    pub stderr_tail: String,
}

/// Errors surfaced to `transcribe_error` (spec §5.3 error table).
#[derive(Debug, Clone)]
pub enum EngineError {
    /// Binary missing or not executable → `errors.engineMissing`.
    Missing(String),
    /// Non-zero exit → `errors.engineFailed`.
    Failed { message: String, stderr_tail: String },
    /// The requested model is not in the catalogue. Distinct from [`Missing`]
    /// because the remediation differs: the user downloads a model rather than
    /// reinstalling the engine (review P2-3).
    ///
    /// [`Missing`]: EngineError::Missing
    ModelNotInstalled(String),
    /// ffmpeg non-zero → `errors.audioExtractFailed`.
    AudioExtractFailed(String),
    /// Spawn/IO failure.
    Io(String),
    /// Malformed WAV input.
    InvalidAudio(String),
    /// CLI produced unparseable output.
    Protocol(String),
}

impl std::fmt::Display for EngineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EngineError::Missing(m) => write!(f, "engine missing: {}", m),
            EngineError::Failed { message, .. } => write!(f, "engine failed: {}", message),
            EngineError::ModelNotInstalled(m) => write!(f, "model not installed: {}", m),
            EngineError::AudioExtractFailed(m) => write!(f, "audio extract failed: {}", m),
            EngineError::Io(m) => write!(f, "io error: {}", m),
            EngineError::InvalidAudio(m) => write!(f, "invalid audio: {}", m),
            EngineError::Protocol(m) => write!(f, "protocol error: {}", m),
        }
    }
}

impl std::error::Error for EngineError {}

/// CLI base name; the bundled file carries a `-<target-triple>` suffix.
const CLI_BINARY: &str = "audiocpp_cli";
/// Env override, mirroring `MONOSUB_FFMPEG_PATH` in `commands::video`.
const CLI_PATH_ENV: &str = "MONOSUB_AUDIOCPP_PATH";
/// `resolve_vad_path` fails without this file (spec §5.1).
const VAD_MODEL_FILE: &str = "silero_vad_16k.safetensors";
/// stderr kept for error reporting (spec §5.1).
const STDERR_TAIL_LINES: usize = 50;

/// Resolve the bundled `audiocpp_cli-<target-triple>` binary.
pub fn resolve_cli_path(app: Option<&tauri::AppHandle>) -> Result<PathBuf, EngineError> {
    resolve_bundled_binary(CLI_BINARY, CLI_PATH_ENV, app)
}

/// Resolve the bundled silero VAD directory
/// (`resources/models/silero_vad`, containing `silero_vad_16k.safetensors`).
pub fn resolve_vad_path(app: Option<&tauri::AppHandle>) -> Result<PathBuf, EngineError> {
    let dir = resolve_resource_path(&Path::new("models").join("silero_vad"), app)?;
    validate_vad_dir(dir)
}

/// Resolve the bundled `models.json` path.
pub fn resolve_catalog_path(app: Option<&tauri::AppHandle>) -> Result<PathBuf, EngineError> {
    resolve_resource_path(Path::new("models.json"), app)
}

/// The CLI silently falls back to its own relative `assets/…` VAD path, so a
/// missing model file has to be an error here rather than a surprise later.
fn validate_vad_dir(dir: PathBuf) -> Result<PathBuf, EngineError> {
    let model_file = dir.join(VAD_MODEL_FILE);
    if !model_file.is_file() {
        return Err(EngineError::Missing(format!(
            "silero VAD model not found at {}",
            model_file.display()
        )));
    }
    Ok(dir)
}

/// Spawn the CLI and call `on_line` for each stdout line as it arrives.
///
/// `stderr` is drained concurrently into a bounded 50-line ring buffer and
/// returned in [`CliOutput::stderr_tail`]. Non-zero exit maps to
/// [`EngineError::Failed`].
pub async fn spawn(
    app: Option<&tauri::AppHandle>,
    invocation: CliInvocation,
    mut on_line: impl FnMut(&str) + Send,
) -> Result<CliOutput, EngineError> {
    let program = resolve_cli_path(app)?;
    let argv = build_argv(&invocation)?;

    tracing::debug!(
        program = %program.display(),
        task = %invocation.task,
        family = %invocation.family,
        backend = ?invocation.backend,
        argv = ?argv,
        "Spawning audiocpp_cli"
    );

    let mut command = tokio::process::Command::new(&program);
    command
        .args(&argv)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);

    let mut child = command.spawn().map_err(|e| {
        tracing::error!(program = %program.display(), error = %e, "Failed to spawn audiocpp_cli");
        EngineError::Missing(format!("failed to spawn {}: {}", program.display(), e))
    })?;

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| EngineError::Io("failed to capture audiocpp_cli stdout".to_string()))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| EngineError::Io("failed to capture audiocpp_cli stderr".to_string()))?;

    // Drain stderr concurrently: the CLI writes to both pipes, and a full
    // stderr pipe would deadlock the child while we read stdout.
    let stderr_task = tokio::spawn(async move {
        let mut reader = BufReader::new(stderr).lines();
        let mut tail: VecDeque<String> = VecDeque::with_capacity(STDERR_TAIL_LINES);
        while let Ok(Some(line)) = reader.next_line().await {
            if tail.len() == STDERR_TAIL_LINES {
                tail.pop_front();
            }
            tail.push_back(line);
        }
        tail.into_iter().collect::<Vec<_>>().join("\n")
    });

    // Forward stdout line-by-line, in order, as lines arrive (P1-9 counts
    // `metrics[<id>].` lines to drive per-chunk progress).
    let mut collected = String::new();
    let mut reader = BufReader::new(stdout).lines();
    while let Some(line) = reader.next_line().await.map_err(|e| {
        tracing::error!(error = %e, "Failed to read audiocpp_cli stdout");
        EngineError::Io(format!("failed to read audiocpp_cli stdout: {}", e))
    })? {
        on_line(&line);
        collected.push_str(&line);
        collected.push('\n');
    }

    let status = child.wait().await.map_err(|e| {
        tracing::error!(error = %e, "Failed to wait for audiocpp_cli");
        EngineError::Io(format!("failed to wait for audiocpp_cli: {}", e))
    })?;

    let stderr_tail = stderr_task
        .await
        .map_err(|e| EngineError::Io(format!("audiocpp_cli stderr reader failed: {}", e)))?;

    let output = CliOutput {
        status,
        stdout: collected,
        stderr_tail,
    };

    if !status.success() {
        tracing::error!(
            task = %invocation.task,
            family = %invocation.family,
            status = ?status.code(),
            stderr_tail = %output.stderr_tail,
            "audiocpp_cli exited with a non-zero status"
        );
        return Err(EngineError::Failed {
            message: format!("audiocpp_cli exited with {}", describe_status(&status)),
            stderr_tail: output.stderr_tail,
        });
    }

    Ok(output)
}

/// Full argv for one invocation.
///
/// Pure so P1-9's command lines can be unit-tested. Common flags come first,
/// then `invocation.args` verbatim, except that a `--request-sequence` run is
/// rejected if it also carries `--request-option`: v0.9.1 only reads
/// `--request-option` on the single-request path, so the flag is never consumed
/// in sequence mode and `require_known_args()` then fails the whole run with
/// `unknown options` (PoC §4.1). Per-request options belong in `requests.json`.
fn build_argv(invocation: &CliInvocation) -> Result<Vec<String>, EngineError> {
    reject_request_option_with_sequence(invocation)?;

    let mut argv = vec![
        "--task".to_string(),
        invocation.task.clone(),
        "--family".to_string(),
        invocation.family.clone(),
        "--model".to_string(),
        invocation.model.to_string_lossy().into_owned(),
        "--backend".to_string(),
        backend_flag(invocation.backend).to_string(),
    ];

    if let Some(audio) = &invocation.audio {
        argv.push("--audio".to_string());
        argv.push(audio.to_string_lossy().into_owned());
    }

    argv.extend(invocation.args.iter().cloned());
    Ok(argv)
}

/// Guard for the PoC §4.1 incompatibility; see [`build_argv`].
fn reject_request_option_with_sequence(invocation: &CliInvocation) -> Result<(), EngineError> {
    let uses_sequence = invocation.args.iter().any(|arg| arg == "--request-sequence");
    if !uses_sequence {
        return Ok(());
    }

    if let Some(offending) = invocation
        .args
        .iter()
        .find(|arg| arg.starts_with("--request-option"))
    {
        return Err(EngineError::Protocol(format!(
            "{} cannot be combined with --request-sequence in audiocpp_cli v0.9.1 \
             (it is rejected as 'unknown options'); put per-request options in the request JSON instead",
            offending
        )));
    }

    Ok(())
}

fn backend_flag(backend: Backend) -> &'static str {
    match backend {
        Backend::Metal => "metal",
        Backend::Cpu => "cpu",
    }
}

/// `errors.engineFailed` detail for a non-zero exit.
fn describe_status(status: &std::process::ExitStatus) -> String {
    match status.code() {
        Some(code) => format!("code {}", code),
        None => "a signal".to_string(),
    }
}

/// Resolution order mirrors `commands::video::resolve_binary_path`:
/// env override → dev checkout → executable dir → app bundle Resources →
/// PATH fallback.
fn resolve_bundled_binary(
    binary_name: &str,
    env_var: &str,
    app: Option<&tauri::AppHandle>,
) -> Result<PathBuf, EngineError> {
    resolve_binary(
        binary_name,
        env_override(env_var),
        app,
        std::env::current_dir().ok().as_deref(),
    )
}

/// [`resolve_bundled_binary`] with the ambient cwd/env override injected, so
/// the dev-checkout branch is testable without mutating the process cwd.
fn resolve_binary(
    binary_name: &str,
    env_path: Option<PathBuf>,
    app: Option<&tauri::AppHandle>,
    cwd: Option<&Path>,
) -> Result<PathBuf, EngineError> {
    if let Some(path) = env_path {
        tracing::debug!(
            binary = binary_name,
            override_path = %path.display(),
            "Using binary path from environment override"
        );
        return Ok(path);
    }

    let candidates = binary_candidates(binary_name);

    // 1. dev checkout: <cwd>/src-tauri/resources/bin
    if let Some(found) = dev_resources_dir(cwd)
        .map(|dir| dir.join("bin"))
        .as_deref()
        .and_then(|dir| find_executable(dir, &candidates))
    {
        tracing::debug!(binary = binary_name, path = ?found, "Found dev binary");
        return Ok(found);
    }

    // 2. packaged layout: next to the executable, then Contents/Resources/resources/bin
    if app.is_some() {
        let exe_path = std::env::current_exe().ok();
        if let Some(found) = exe_path
            .as_deref()
            .and_then(Path::parent)
            .and_then(|exe_dir| find_executable(exe_dir, &candidates))
        {
            tracing::debug!(binary = binary_name, path = ?found, "Found bundled binary");
            return Ok(found);
        }

        if let Some(found) = exe_path
            .as_deref()
            .and_then(bundle_resources_dir)
            .map(|dir| dir.join("bin"))
            .as_deref()
            .and_then(|dir| find_executable(dir, &candidates))
        {
            tracing::debug!(binary = binary_name, path = ?found, "Found bundled resource binary");
            return Ok(found);
        }
    }

    tracing::warn!(
        binary = binary_name,
        "audiocpp_cli not found; falling back to the bare name in PATH"
    );
    Ok(PathBuf::from(binary_name))
}

/// `<root>/src-tauri/resources`, when `<root>` is a dev checkout.
fn dev_resources_dir(root: Option<&Path>) -> Option<PathBuf> {
    let dir = root?.join("src-tauri").join("resources");
    dir.is_dir().then_some(dir)
}

/// `Contents/Resources/resources` for a running executable.
fn bundle_resources_dir(exe_path: &Path) -> Option<PathBuf> {
    let contents_dir = exe_path.parent()?.parent()?;
    Some(contents_dir.join("Resources").join("resources"))
}

/// First candidate that exists under `dir` and is executable.
fn find_executable(dir: &Path, candidates: &[String]) -> Option<PathBuf> {
    candidates
        .iter()
        .map(|candidate| dir.join(candidate))
        .find(|path| is_executable_file(path))
}

/// A bundled resource: dev checkout first, then the app bundle.
fn resolve_resource_path(
    relative: &Path,
    app: Option<&tauri::AppHandle>,
) -> Result<PathBuf, EngineError> {
    let cwd = std::env::current_dir().ok();
    if let Some(path) = dev_resources_dir(cwd.as_deref())
        .map(|dir| dir.join(relative))
        .filter(|path| path.exists())
    {
        tracing::debug!(path = ?path, "Found dev resource");
        return Ok(path);
    }

    if app.is_some() {
        if let Some(path) = std::env::current_exe()
            .ok()
            .as_deref()
            .and_then(bundle_resources_dir)
            .map(|dir| dir.join(relative))
            .filter(|path| path.exists())
        {
            tracing::debug!(path = ?path, "Found bundled resource");
            return Ok(path);
        }
    }

    Err(EngineError::Missing(format!(
        "bundled resource {} not found",
        relative.display()
    )))
}

fn env_override(env_var: &str) -> Option<PathBuf> {
    std::env::var(env_var)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .map(PathBuf::from)
}

/// Base name first, then the `-<target-triple>` variant (video.rs convention).
fn binary_candidates(binary_name: &str) -> Vec<String> {
    let target_suffix = target_triple();
    let mut candidates = vec![binary_name.to_string()];
    if !target_suffix.is_empty() {
        candidates.push(format!("{}-{}", binary_name, target_suffix));
    }
    candidates
}

/// Target triple used for bundled binary file names (mirrors `video.rs`).
fn target_triple() -> String {
    #[cfg(all(target_arch = "aarch64", target_os = "macos"))]
    return "aarch64-apple-darwin".to_string();

    #[cfg(all(target_arch = "x86_64", target_os = "macos"))]
    return "x86_64-apple-darwin".to_string();

    #[cfg(all(target_arch = "x86_64", target_os = "linux"))]
    return "x86_64-unknown-linux-gnu".to_string();

    #[cfg(all(target_arch = "aarch64", target_os = "linux"))]
    return "aarch64-unknown-linux-gnu".to_string();

    #[cfg(all(target_arch = "x86_64", target_os = "windows"))]
    return "x86_64-pc-windows-msvc".to_string();

    #[cfg(not(any(
        all(target_arch = "aarch64", target_os = "macos"),
        all(target_arch = "x86_64", target_os = "macos"),
        all(target_arch = "x86_64", target_os = "linux"),
        all(target_arch = "aarch64", target_os = "linux"),
        all(target_arch = "x86_64", target_os = "windows")
    )))]
    return String::new();
}

fn is_executable_file(path: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::metadata(path)
            .map(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
            .unwrap_or(false)
    }
    #[cfg(not(unix))]
    {
        path.is_file()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::vad::{VAD_MAX_SECONDS, VAD_MERGE_GAP_SECONDS, VAD_PADDING_SECONDS};

    /// The dev checkout root, derived from the crate location rather than the
    /// process cwd (`cargo test` runs with `src-tauri` as cwd).
    fn repo_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("crate lives in <repo>/src-tauri")
            .to_path_buf()
    }

    fn committed_cli_path() -> PathBuf {
        repo_root()
            .join("src-tauri")
            .join("resources")
            .join("bin")
            .join(format!("{}-{}", CLI_BINARY, target_triple()))
    }

    fn vad_invocation() -> CliInvocation {
        CliInvocation {
            task: "vad".to_string(),
            family: "silero_vad".to_string(),
            model: PathBuf::from("/res/models/silero_vad"),
            audio: Some(PathBuf::from("/tmp/monosub-t1/audio.wav")),
            backend: Backend::Cpu,
            args: vec![
                "--vad-chunks-out".to_string(),
                "/tmp/monosub-t1/vad_chunks.json".to_string(),
                "--vad-chunk-max-seconds".to_string(),
                VAD_MAX_SECONDS.to_string(),
                "--vad-chunk-merge-gap-seconds".to_string(),
                VAD_MERGE_GAP_SECONDS.to_string(),
                "--vad-chunk-padding-seconds".to_string(),
                VAD_PADDING_SECONDS.to_string(),
            ],
        }
    }

    fn asr_sequence_invocation() -> CliInvocation {
        CliInvocation {
            task: "asr".to_string(),
            family: "sense_asr".to_string(),
            model: PathBuf::from("/models/SenseVoice-Small-GGUF/sensevoice-small-q8-audiocpp-v1.gguf"),
            audio: None,
            backend: Backend::Metal,
            args: vec![
                "--request-sequence".to_string(),
                "/tmp/monosub-t1/requests.json".to_string(),
                "--text-out".to_string(),
                "/tmp/monosub-t1/out/seg.txt".to_string(),
                "--session-option".to_string(),
                "sense_asr.vad_model_path=/res/models/silero_vad".to_string(),
                "--threads".to_string(),
                "8".to_string(),
                "--metrics".to_string(),
            ],
        }
    }

    fn arg_after<'a>(argv: &'a [String], flag: &str) -> Option<&'a str> {
        let index = argv.iter().position(|arg| arg == flag)?;
        argv.get(index + 1).map(String::as_str)
    }

    #[test]
    fn argv_for_vad_invocation() {
        let argv = build_argv(&vad_invocation()).expect("vad argv builds");

        assert_eq!(
            argv,
            vec![
                "--task",
                "vad",
                "--family",
                "silero_vad",
                "--model",
                "/res/models/silero_vad",
                "--backend",
                "cpu",
                "--audio",
                "/tmp/monosub-t1/audio.wav",
                "--vad-chunks-out",
                "/tmp/monosub-t1/vad_chunks.json",
                "--vad-chunk-max-seconds",
                VAD_MAX_SECONDS.to_string().as_str(),
                "--vad-chunk-merge-gap-seconds",
                VAD_MERGE_GAP_SECONDS.to_string().as_str(),
                "--vad-chunk-padding-seconds",
                VAD_PADDING_SECONDS.to_string().as_str(),
            ]
        );
    }

    #[test]
    fn argv_for_asr_request_sequence_invocation() {
        let argv = build_argv(&asr_sequence_invocation()).expect("sequence argv builds");

        assert_eq!(arg_after(&argv, "--task"), Some("asr"));
        assert_eq!(arg_after(&argv, "--family"), Some("sense_asr"));
        assert_eq!(
            arg_after(&argv, "--model"),
            Some("/models/SenseVoice-Small-GGUF/sensevoice-small-q8-audiocpp-v1.gguf")
        );
        assert_eq!(arg_after(&argv, "--backend"), Some("metal"));
        // No `--audio`: sequence mode names every chunk inside requests.json.
        assert!(!argv.iter().any(|arg| arg == "--audio"));
        assert_eq!(
            arg_after(&argv, "--request-sequence"),
            Some("/tmp/monosub-t1/requests.json")
        );
        assert_eq!(
            arg_after(&argv, "--session-option"),
            Some("sense_asr.vad_model_path=/res/models/silero_vad")
        );
        assert!(argv.iter().any(|arg| arg == "--metrics"));
    }

    /// PoC §4.1: v0.9.1 rejects `--request-option` together with
    /// `--request-sequence` (`unknown options`), so the ASR sequence argv must
    /// never contain it — per-request options live in `requests.json`.
    #[test]
    fn asr_sequence_argv_never_passes_request_option() {
        let argv = build_argv(&asr_sequence_invocation()).expect("sequence argv builds");

        // Guard against a vacuous pass: the argv really is the sequence form.
        assert!(argv.iter().any(|arg| arg == "--request-sequence"), "{argv:?}");
        assert!(argv.iter().any(|arg| arg == "--metrics"), "{argv:?}");

        assert!(
            !argv.iter().any(|arg| arg.starts_with("--request-option")),
            "unexpected --request-option in {argv:?}"
        );
    }

    /// [`build_argv`] rejects the PoC §4.1 combination outright, so a future
    /// P1-9 change cannot silently reintroduce the failing command line.
    #[test]
    fn build_argv_rejects_request_option_with_request_sequence() {
        let mut invocation = asr_sequence_invocation();
        invocation.args.push("--request-option".to_string());
        invocation.args.push("language=zh".to_string());

        match build_argv(&invocation) {
            Err(EngineError::Protocol(message)) => {
                assert!(
                    message.contains("--request-option") && message.contains("--request-sequence"),
                    "unhelpful message: {message}"
                );
            }
            other => panic!("expected Protocol rejection, got {other:?}"),
        }
    }

    /// The single-request path may still use `--request-option` (spec §5.5).
    #[test]
    fn build_argv_allows_request_option_without_sequence() {
        let invocation = CliInvocation {
            task: "asr".to_string(),
            family: "qwen3_asr".to_string(),
            model: PathBuf::from("/models/qwen3-asr.gguf"),
            audio: Some(PathBuf::from("/tmp/audio.wav")),
            backend: Backend::Metal,
            args: vec![
                "--words-out".to_string(),
                "/tmp/words.json".to_string(),
                "--request-option".to_string(),
                "preserve_punctuation=true".to_string(),
            ],
        };

        let argv = build_argv(&invocation).expect("single-request argv builds");
        assert_eq!(arg_after(&argv, "--request-option"), Some("preserve_punctuation=true"));
    }

    /// The committed checkout must contain the binary under the exact
    /// `audiocpp_cli-<target-triple>` name that resolution looks for.
    #[test]
    fn dev_bin_dir_resolves_committed_cli_binary() {
        let dir = dev_resources_dir(Some(&repo_root()))
            .expect("repo root is a dev checkout")
            .join("bin");
        let found = find_executable(&dir, &binary_candidates(CLI_BINARY))
            .expect("committed audiocpp_cli is discoverable");

        assert_eq!(found, committed_cli_path());
        assert!(is_executable_file(&found), "{}", found.display());
    }

    /// `resolve_cli_path`'s dev branch, driven from the repo root so the result
    /// does not depend on the test process cwd.
    #[test]
    fn resolve_binary_finds_committed_cli_in_dev_checkout() {
        let resolved = resolve_binary(CLI_BINARY, None, None, Some(&repo_root()))
            .expect("dev checkout resolution succeeds");

        assert_eq!(resolved, committed_cli_path());
        assert!(resolved.is_file(), "{}", resolved.display());
        assert!(is_executable_file(&resolved), "{}", resolved.display());
    }

    /// The env override short-circuits the search, as in `commands::video`.
    #[test]
    fn resolve_binary_prefers_env_override() {
        let override_path = PathBuf::from("/nowhere/audiocpp_cli");
        let resolved = resolve_binary(CLI_BINARY, Some(override_path.clone()), None, Some(&repo_root()));

        assert_eq!(resolved.unwrap(), override_path);
    }

    /// With no bundled candidate and no app bundle the name is passed through
    /// so the OS can search PATH, matching `commands::video`.
    #[test]
    fn resolve_binary_falls_back_to_bare_name() {
        let resolved = resolve_binary(CLI_BINARY, None, None, None).unwrap();

        assert_eq!(resolved, PathBuf::from(CLI_BINARY));
    }

    #[test]
    fn dev_resources_dir_resolves_committed_vad_and_catalog() {
        let resources = dev_resources_dir(Some(&repo_root())).expect("dev checkout resources");

        let vad_dir = validate_vad_dir(resources.join("models").join("silero_vad"))
            .expect("bundled silero VAD resolves");
        assert!(vad_dir.join(VAD_MODEL_FILE).is_file(), "{:?}", vad_dir);

        assert!(resources.join("models.json").is_file());
    }

    /// Without `silero_vad_16k.safetensors` the CLI would silently fall back to
    /// its own relative VAD path, so resolution must fail here instead.
    #[test]
    fn validate_vad_dir_errors_without_model_file() {
        let empty_dir =
            std::env::temp_dir().join(format!("monosub-vad-empty-{}", std::process::id()));
        std::fs::create_dir_all(&empty_dir).unwrap();

        assert!(matches!(
            validate_vad_dir(empty_dir.clone()),
            Err(EngineError::Missing(_))
        ));

        std::fs::write(empty_dir.join(VAD_MODEL_FILE), b"stub").unwrap();
        assert_eq!(validate_vad_dir(empty_dir.clone()).unwrap(), empty_dir);

        std::fs::remove_dir_all(&empty_dir).unwrap();
    }
}

#[cfg(test)]
mod spawn_tests {
    use super::*;
    use std::path::{Path as P, PathBuf};
    use std::sync::Mutex;

    /// `spawn` resolves the binary through a process-global env override, so
    /// these probes serialise themselves and restore the previous value.
    static SPAWN_LOCK: Mutex<()> = Mutex::new(());

    struct EnvGuard {
        key: &'static str,
        previous: Option<std::ffi::OsString>,
    }

    impl EnvGuard {
        fn set(key: &'static str, value: &P) -> Self {
            let previous = std::env::var_os(key);
            std::env::set_var(key, value);
            Self { key, previous }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            match &self.previous {
                Some(v) => std::env::set_var(self.key, v),
                None => std::env::remove_var(self.key),
            }
        }
    }

    fn repo_root() -> PathBuf {
        P::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("crate lives in <repo>/src-tauri")
            .to_path_buf()
    }

    fn resources() -> PathBuf {
        repo_root().join("src-tauri").join("resources")
    }

    fn vad_dir() -> PathBuf {
        resources().join("models").join("silero_vad")
    }

    /// The bundled binary and model are gitignored, so skip with a reason elsewhere.
    fn bundled_cli() -> Option<PathBuf> {
        let cli = find_executable(&resources().join("bin"), &binary_candidates(CLI_BINARY));
        let vad_ok = vad_dir().join(VAD_MODEL_FILE).is_file();
        match (cli, vad_ok) {
            (Some(cli), true) => Some(cli),
            _ => {
                eprintln!("skip: bundled audiocpp_cli / silero VAD not present");
                None
            }
        }
    }

    fn silence_wav(path: &P, seconds: usize) {
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: 16_000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut writer = hound::WavWriter::create(path, spec).expect("create wav");
        for _ in 0..16_000 * seconds {
            writer.write_sample(0i16).expect("sample");
        }
        writer.finalize().expect("finalize wav");
    }

    fn vad_invocation(audio: PathBuf, dir: &P) -> CliInvocation {
        CliInvocation {
            task: "vad".to_string(),
            family: "silero_vad".to_string(),
            model: vad_dir(),
            audio: Some(audio),
            backend: Backend::Cpu,
            args: vec![
                "--vad-chunks-out".to_string(),
                dir.join("c.json").to_string_lossy().into_owned(),
                "--vad-chunk-max-seconds".to_string(),
                "15".to_string(),
            ],
        }
    }

    /// Regression cover for the half of [`spawn`] that had no executable
    /// evidence: stdout must be forwarded line-by-line, in order, while also
    /// being collected, and a clean run must return `Ok`.
    #[tokio::test]
    async fn spawn_forwards_stdout_in_order_and_reports_success() {
        let Some(cli) = bundled_cli() else { return };
        let _guard = SPAWN_LOCK.lock().unwrap();
        let _env = EnvGuard::set(CLI_PATH_ENV, &cli);

        let dir = std::env::temp_dir().join(format!("monosub-spawn-ok-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).expect("create dir");
        let wav = dir.join("silence.wav");
        silence_wav(&wav, 3);

        let mut lines: Vec<String> = Vec::new();
        let output = spawn(None, vad_invocation(wav, &dir), |line| lines.push(line.to_string()))
            .await
            .expect("VAD on digital silence must succeed");

        assert!(output.status.success(), "status: {:?}", output.status);
        assert!(!lines.is_empty(), "stdout must reach on_line");
        // Exactly the collected lines, in the same order.
        assert_eq!(
            lines,
            output.stdout.lines().collect::<Vec<_>>(),
            "on_line order must match stdout"
        );
        assert!(
            output.stdout.contains("vad_chunks_out"),
            "unexpected stdout: {}",
            output.stdout
        );

        let parsed = crate::engine::vad::parse_chunks(
            &std::fs::read_to_string(dir.join("c.json")).expect("chunk plan"),
        )
        .expect("parse chunks");
        assert_eq!(parsed, Vec::new(), "silence yields no chunks with exit 0");

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The other half: a non-zero exit becomes [`EngineError::Failed`] and keeps
    /// the captured stderr tail for the error report.
    #[tokio::test]
    async fn spawn_maps_nonzero_exit_to_failed_with_stderr_tail() {
        let Some(cli) = bundled_cli() else { return };
        let _guard = SPAWN_LOCK.lock().unwrap();
        let _env = EnvGuard::set(CLI_PATH_ENV, &cli);

        let dir = std::env::temp_dir().join(format!("monosub-spawn-bad-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).expect("create dir");

        // A WAV path that does not exist: the CLI cannot open the input.
        let error = spawn(None, vad_invocation(dir.join("nope.wav"), &dir), |_| {})
            .await
            .expect_err("a missing WAV must fail");

        match error {
            EngineError::Failed { stderr_tail, .. } => {
                assert!(!stderr_tail.is_empty(), "stderr tail must be captured");
            }
            other => panic!("expected EngineError::Failed, got {other:?}"),
        }

        let _ = std::fs::remove_dir_all(&dir);
    }
}
