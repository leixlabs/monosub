//! audio.cpp inference engine orchestration (see docs/tech-spec/refactor-audiocpp.md §4).
//!
//! Owning tasks: P1-5..P1-9.
//!
//! Pinned baseline — the exports below are the frozen interface between the
//! `engine` module (P1-5..P1-9) and `commands/transcribe.rs` (P1-15).

pub mod catalog;
pub mod cli;
pub mod diar;
pub mod direct;
pub mod plan;
pub mod vad;
pub mod wav;

use std::path::PathBuf;

pub use cli::{Backend, CliInvocation, CliOutput, EngineError};

/// One VAD segment or ASR transcript with absolute media-relative times.
#[derive(Debug, Clone, PartialEq)]
pub struct RawChunk {
    pub start_s: f64,
    pub end_s: f64,
    pub text: String,
    /// Already-formatted speaker label (`Speaker A`), or `None` when the source
    /// produced no speaker information (design §6.5).
    pub speaker: Option<String>,
    /// The ASR's own language tag (`<|zh|>` → `zh`), extracted before
    /// `subtitle::postprocess::clean_text` deletes it (translation-design §5).
    /// `None` when the source produced no tag — the passthrough model does not,
    /// and that means "unknown", never an error.
    pub language: Option<String>,
}

/// Options for one file's transcription run.
#[derive(Debug, Clone)]
pub struct TranscribeOpts {
    /// App language (`zh`, `zh-tw`, `en`, …); mapped via `catalog::map_language`.
    pub language: String,
    /// Model id from `models.json`; `None` uses the catalogue default.
    pub model_id: Option<String>,
    /// Requested backend; `auto` resolves to metal on macOS with a cpu retry.
    pub backend: Backend,
}

impl Default for TranscribeOpts {
    fn default() -> Self {
        Self {
            language: "auto".to_string(),
            model_id: None,
            backend: Backend::Metal,
        }
    }
}

/// Progress callback payload: `(done, total, message)`.
pub type ProgressFn<'a> = dyn Fn(usize, usize, &str) + Send + Sync + 'a;

/// audio.cpp task for ASR. The `family` always comes from the selected model's
/// `ModelSpec`, never from a constant here: a GGUF embeds its own family and
/// `audiocpp_cli` rejects a mismatch with
/// "GGUF embeds model spec for family 'X', not 'Y'".
const ASR_TASK: &str = "asr";

/// Per-request option written into `requests.json`.
///
/// PoC §4.1: v0.9.1 rejects `--request-option` together with `--request-sequence`
/// (`unknown options`), so every per-request option has to travel inside the
/// JSON document instead of on the command line.
const REQUEST_CHUNK_MODE: &str = "none";

/// Stem of the ASR `--text-out` path. The CLI appends `_<safe_id>` plus `.txt`
/// and prints `text_out[<id>]=<path>`, which we prefer over guessing the name.
const TEXT_OUT_STEM: &str = "seg.txt";

/// How many chunks go into one `--request-sequence` invocation (spec §7 risk 9).
const MAX_CHUNKS_PER_BATCH: usize = 200;

/// Run the full pipeline for one file: VAD → slice → `requests.json` →
/// `--request-sequence` ASR → per-chunk text.
///
/// Temporary files live in `temp_dir()/monosub-<task_id>/` and are removed on
/// all exits (success, failure, cancel) via a `Drop` guard.
///
/// Task: P1-9. Cancellation is handled by the caller aborting the owning tokio
/// task: `cli::spawn` marks the child `kill_on_drop`, so aborting drops the
/// future and kills the child process.
pub async fn transcribe(
    app: &tauri::AppHandle,
    task_id: &str,
    wav_path: &std::path::Path,
    opts: &TranscribeOpts,
    on_progress: &ProgressFn<'_>,
) -> Result<Vec<RawChunk>, EngineError> {
    let work_dir = TempWorkDir::create(task_id)?;
    let dir = work_dir.path();

    // Resolve what to run: `--model` is the GGUF file for ASR, the VAD
    // directory is passed through `--session-option`.
    let model_id = opts
        .model_id
        .clone()
        .unwrap_or_else(|| catalog::default_model_id().to_string());
    let model_file = catalog::model_path(app, &model_id)?.ok_or_else(|| {
        EngineError::ModelNotInstalled(format!("model '{}' is not in the catalogue", model_id))
    })?;
    let specs = catalog::catalog(Some(app))?;
    let spec = specs.iter().find(|m| m.id == model_id).ok_or_else(|| {
        EngineError::ModelNotInstalled(format!("model '{}' is not in the catalogue", model_id))
    })?;
    let language = catalog::map_language(spec, &opts.language);

    // Step 2: VAD. Cheap (no model load) and decides everything downstream.
    on_progress(0, 0, "Detecting speech");
    let chunks = vad::detect(Some(app), wav_path, dir).await?;

    // All-silent input: exit code 0 with zero chunks (PoC §4.3). Not an error —
    // the caller writes an empty subtitle.
    if chunks.is_empty() {
        tracing::info!(task_id = %task_id, "VAD found no speech; writing an empty subtitle");
        on_progress(0, 0, "No speech detected");
        return Ok(Vec::new());
    }

    let total = chunks.len();
    tracing::info!(task_id = %task_id, chunks = total, "VAD chunk plan ready");

    // Step 3a: slice the 16 kHz mono WAV at the sample offsets VAD reported.
    let audio = wav::read(wav_path)?;
    on_progress(0, total, "Segmenting audio");
    for chunk in &chunks {
        let out_path = wav::chunk_path(dir, chunk.index);
        wav::slice(&audio, chunk.start_sample, chunk.end_sample, &out_path)?;
    }

    // Step 3b: transcribe. Each batch is one session, so the model loads once.
    let mut texts: Vec<String> = Vec::with_capacity(total);
    let mut done = 0usize;
    for batch in chunks.chunks(MAX_CHUNKS_PER_BATCH) {
        let results = transcribe_batch(
            app,
            dir,
            &model_file,
            // The family comes from the SELECTED model, never a constant: the
            // solver may pick `qwen3_asr` for es/de/fr/pt or a manual override,
            // and the GGUF must be loaded by its own family.
            &spec.family,
            &language,
            opts.backend,
            batch,
            total,
            done,
            on_progress,
        )
        .await?;

        for chunk in batch {
            texts.push(results.get(&chunk.index).cloned().unwrap_or_default());
        }
        done += batch.len();
        on_progress(done, total, "Transcribing");
    }

    // Step 4 is `subtitle::postprocess`; keep the chunk times intact here so it
    // can distribute each chunk's duration proportionally.
    let out = chunks
        .iter()
        .zip(texts)
        .map(|(chunk, text)| RawChunk {
            start_s: chunk.start_s(),
            end_s: chunk.end_s(),
            // The language tag must be read BEFORE it is cleaned away further
            // down the pipeline (translation-design §5); `clean_text` runs in
            // `subtitle::postprocess`, not here, so `text` is still raw.
            language: crate::subtitle::postprocess::extract_language_tag(&text),
            text,
            // The VAD-sequence path has no speaker model, so no label here.
            speaker: None,
        })
        .collect();

    Ok(out)
}

/// Transcribe one batch of chunks in a single CLI session.
///
/// Retries once on the CPU backend when a `metal` run fails with a backend-ish
/// error (spec §5.3).
#[allow(clippy::too_many_arguments)]
async fn transcribe_batch(
    app: &tauri::AppHandle,
    dir: &std::path::Path,
    model_file: &std::path::Path,
    family: &str,
    language: &str,
    backend: Backend,
    batch: &[vad::VadChunk],
    total: usize,
    done_before: usize,
    on_progress: &ProgressFn<'_>,
) -> Result<std::collections::HashMap<usize, String>, EngineError> {
    let vad_dir = cli::resolve_vad_path(Some(app))?;
    let requests_path = write_requests(dir, batch, language)?;

    // `done` may only ever increase for a given file. A Metal attempt can emit
    // metrics and *then* fail, so without a shared floor the retry would report a
    // lower value and the progress bar would visibly run backwards. One
    // monotonic floor across both attempts makes that impossible.
    let floor = std::sync::atomic::AtomicUsize::new(done_before);
    // Same 3-arg shape as `ProgressFn`, so `run_sequence` stays unchanged.
    let report = |done: usize, total: usize, message: &str| {
        if should_report(&floor, done, total) {
            on_progress(done.min(total), total, message);
        }
    };

    match run_sequence(
        app, dir, model_file, family, &vad_dir, &requests_path, backend, batch, total, done_before,
        &report,
    )
    .await
    {
        Ok(results) => Ok(results),
        Err(err @ EngineError::Failed { .. }) if backend == Backend::Metal && is_backend_failure(&err) => {
            tracing::warn!(
                error = %err,
                "metal backend failed; retrying once on cpu"
            );
            // Announce only if the notice itself would not move progress back.
            if floor.load(std::sync::atomic::Ordering::SeqCst) <= done_before {
                on_progress(done_before, total, "Metal unavailable, retrying on CPU");
            }
            run_sequence(
                app, dir, model_file, family, &vad_dir, &requests_path, Backend::Cpu, batch, total,
                done_before, &report,
            )
            .await
        }
        Err(err) => Err(err),
    }
}

/// True when `done` advances the monotonic floor, recording the new high-water
/// mark. Extracted so the "progress never regresses" rule is unit-testable
/// without an `AppHandle`.
fn should_report(
    floor: &std::sync::atomic::AtomicUsize,
    done: usize,
    total: usize,
) -> bool {
    let done = done.min(total);
    // `fetch_max` returns the previous value, so this fires only on an increase.
    floor.fetch_max(done, std::sync::atomic::Ordering::SeqCst) < done
}

/// One `--request-sequence` invocation plus its `text_out` files.
#[allow(clippy::too_many_arguments)]
async fn run_sequence(
    app: &tauri::AppHandle,
    dir: &std::path::Path,
    model_file: &std::path::Path,
    family: &str,
    vad_dir: &std::path::Path,
    requests_path: &std::path::Path,
    backend: Backend,
    batch: &[vad::VadChunk],
    total: usize,
    done_before: usize,
    on_progress: &ProgressFn<'_>,
) -> Result<std::collections::HashMap<usize, String>, EngineError> {
    let out_dir = dir.join("out");
    let text_out = out_dir.join(TEXT_OUT_STEM);

    let invocation = CliInvocation {
        task: ASR_TASK.to_string(),
        family: family.to_string(),
        model: model_file.to_path_buf(),
        // Sequence mode names every chunk inside requests.json.
        audio: None,
        backend,
        args: asr_args(requests_path, &text_out, vad_dir, family),
    };

    // Ids are `c0000`-style, so the CLI's `safe_output_name` leaves them alone.
    let expected: std::collections::HashMap<String, usize> = batch
        .iter()
        .map(|chunk| (chunk_id(chunk.index), chunk.index))
        .collect();

    let mut reported: std::collections::HashMap<String, PathBuf> = std::collections::HashMap::new();
    let mut metrics_seen = 0usize;
    // Seeded from `done_before`, not 0: on a Metal→CPU retry some metrics may
    // already have advanced inside the failed attempt, so starting at 0 would
    // re-emit an already-reported value and let the percentage visibly regress.
    let mut last_reported_done = done_before;

    {
        let mut on_line = |line: &str| {
            // `text_out[<id>]=<path>` — the authoritative output path. It is
            // absolute when `--text-out` was absolute (always the case here),
            // but the CLI echoes whatever we passed, so resolve defensively
            // against the process cwd the child inherited.
            if let Some((id, path)) = parse_text_out(line) {
                reported.insert(id, resolve_reported_path(&path));
                return;
            }
            // `metrics[<id>].…` — one block per finished chunk, streamed live
            // (PoC §4.2), which is what drives progress.
            if let Some(id) = parse_metric_id(line) {
                if let Some(&index) = expected.get(&id) {
                    // Guard against counting a duplicate id twice.
                    if !reported.contains_key(&format!("__metric__{}", index)) {
                        reported.insert(format!("__metric__{}", index), PathBuf::new());
                        metrics_seen += 1;
                        let done = (done_before + metrics_seen).min(total);
                        if done != last_reported_done {
                            last_reported_done = done;
                            on_progress(done, total, "Transcribing");
                        }
                    }
                } else {
                    tracing::debug!(line = %line, "metrics line for an unknown request id");
                }
            }
        };

        let output: CliOutput = cli::spawn(Some(app), invocation, &mut on_line).await?;
        tracing::debug!(
            stdout_lines = output.stdout.lines().count(),
            stderr_tail = %output.stderr_tail,
            "audiocpp_cli sequence finished"
        );
    }

    // Read each chunk's text, preferring the path the CLI printed.
    let mut results = std::collections::HashMap::with_capacity(batch.len());
    let mut missing = Vec::new();
    for chunk in batch {
        let id = chunk_id(chunk.index);
        let path = reported
            .get(&id)
            .cloned()
            .unwrap_or_else(|| default_text_out_path(&text_out, &id));

        match std::fs::read_to_string(&path) {
            Ok(text) => {
                results.insert(chunk.index, text.trim().to_string());
            }
            Err(e) => {
                tracing::warn!(
                    chunk = chunk.index,
                    path = %path.display(),
                    error = %e,
                    "no transcript written for chunk; treating as silent"
                );
                missing.push(chunk.index);
            }
        }
    }

    if missing.len() == batch.len() {
        return Err(EngineError::Protocol(format!(
            "audiocpp_cli reported success but wrote no transcript for any of the {} chunk(s)",
            batch.len()
        )));
    }

    Ok(results)
}

/// `--request-sequence` arguments. Deliberately contains no `--request-option`
/// (PoC §4.1).
///
/// `family` also namespaces the session option: `sense_asr.vad_model_path` and
/// `qwen3_asr.vad_model_path` are different keys, so the prefix must be the
/// selected model's family too. Only families that actually take a VAD path get
/// the option; the rest would see an unknown key.
fn asr_args(
    requests_path: &std::path::Path,
    text_out: &std::path::Path,
    vad_dir: &std::path::Path,
    family: &str,
) -> Vec<String> {
    let mut args = vec![
        "--request-sequence".to_string(),
        requests_path.to_string_lossy().into_owned(),
        "--text-out".to_string(),
        text_out.to_string_lossy().into_owned(),
    ];

    if family_takes_vad_model_path(family) {
        args.push("--session-option".to_string());
        args.push(format!("{}.vad_model_path={}", family, vad_dir.to_string_lossy()));
    }

    args.push("--threads".to_string());
    args.push(thread_count().to_string());
    args.push("--metrics".to_string());
    args
}

/// Families whose spec declares a `vad_model_path` session option.
///
/// A family without it is not the same as "no VAD": the option is opt-in and
/// passing it under an unknown family/key would be ignored at best. Verified
/// against `audiocpp_cli --list-loaders --json` for v0.9.1.
fn family_takes_vad_model_path(family: &str) -> bool {
    matches!(family, "sense_asr" | "fun_asr_nano")
}

/// Write `requests.json` for one batch.
///
/// Relative `audio` paths resolve against the document's directory (PoC §4.2),
/// so the bare chunk file name is enough — everything lives in `dir`.
fn write_requests(
    dir: &std::path::Path,
    batch: &[vad::VadChunk],
    language: &str,
) -> Result<PathBuf, EngineError> {
    let requests: Vec<serde_json::Value> = batch
        .iter()
        .map(|chunk| {
            serde_json::json!({
                "id": chunk_id(chunk.index),
                "audio": wav::chunk_file_name(chunk.index),
                "language": language,
                "options": { "audio_chunk_mode": REQUEST_CHUNK_MODE },
            })
        })
        .collect();

    let document = serde_json::json!({ "requests": requests });
    let path = dir.join("requests.json");
    let body = serde_json::to_string(&document)
        .map_err(|e| EngineError::Protocol(format!("failed to serialize requests.json: {}", e)))?;
    std::fs::write(&path, body)
        .map_err(|e| EngineError::Io(format!("write {}: {}", path.display(), e)))?;
    Ok(path)
}

/// Request id for one chunk; already `safe_output_name`-clean.
fn chunk_id(index: usize) -> String {
    format!("c{:04}", index)
}

/// Fallback path when the CLI did not print `text_out[<id>]`.
///
/// Mirrors `safe_output_name`: `<stem>_<safe_id><ext>`.
fn default_text_out_path(text_out: &std::path::Path, id: &str) -> PathBuf {
    unsafe_output_name(text_out, id)
}

fn unsafe_output_name(text_out: &std::path::Path, id: &str) -> PathBuf {
    let stem = text_out
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "seg".to_string());
    let safe_id: String = id
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let ext = text_out
        .extension()
        .map(|e| e.to_string_lossy().into_owned())
        .unwrap_or_else(|| "txt".to_string());
    let name = format!("{}_{}.{}", stem, safe_id, ext);
    match text_out.parent() {
        Some(parent) => parent.join(name),
        None => PathBuf::from(name),
    }
}

/// Parse `text_out[<id>]=<path>`.
fn parse_text_out(line: &str) -> Option<(String, String)> {
    let rest = line.strip_prefix("text_out[")?;
    let (id, path) = rest.split_once("]=")?;
    Some((id.to_string(), path.trim().to_string()))
}

/// The CLI echoes the `--text-out` path it was given, so a relative path is
/// relative to the cwd the child inherited from us.
fn resolve_reported_path(path: &str) -> PathBuf {
    let path = PathBuf::from(path);
    if path.is_absolute() {
        return path;
    }
    match std::env::current_dir() {
        Ok(cwd) => cwd.join(path),
        Err(_) => path,
    }
}

/// Parse the request id out of `metrics[<id>].<key>=<value>`.
fn parse_metric_id(line: &str) -> Option<String> {
    let rest = line.strip_prefix("metrics[")?;
    let (id, _) = rest.split_once(']')?;
    Some(id.to_string())
}

/// A `metal` run that failed for backend reasons is worth one CPU retry.
fn is_backend_failure(err: &EngineError) -> bool {
    let EngineError::Failed { stderr_tail, message } = err else {
        return false;
    };
    let haystack = format!("{}\n{}", message, stderr_tail).to_lowercase();
    haystack.contains("metal") || haystack.contains("backend") || haystack.contains("gpu")
}

/// Physical cores, used for `--threads`.
fn thread_count() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
}

/// `temp_dir()/monosub-<task_id>/`, removed on drop for every exit path.
pub(super) struct TempWorkDir {
    path: PathBuf,
}

impl TempWorkDir {
    pub(super) fn create(task_id: &str) -> Result<Self, EngineError> {
        let safe: String = task_id
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                    c
                } else {
                    '_'
                }
            })
            .collect();
        let path = std::env::temp_dir().join(format!("monosub-{}", safe));

        // A leftover directory from a crashed run would poison this one.
        if path.exists() {
            let _ = std::fs::remove_dir_all(&path);
        }
        std::fs::create_dir_all(&path)
            .map_err(|e| EngineError::Io(format!("create {}: {}", path.display(), e)))?;

        Ok(Self { path })
    }

    pub(super) fn path(&self) -> &std::path::Path {
        &self.path
    }
}

impl Drop for TempWorkDir {
    fn drop(&mut self) {
        if let Err(e) = std::fs::remove_dir_all(&self.path) {
            tracing::warn!(
                path = %self.path.display(),
                error = %e,
                "failed to remove the transcription temp directory"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_text_out_line() {
        assert_eq!(
            parse_text_out("text_out[c0000]=/tmp/monosub-t1/out/seg_c0000.txt"),
            Some((
                "c0000".to_string(),
                "/tmp/monosub-t1/out/seg_c0000.txt".to_string()
            ))
        );
        assert_eq!(parse_text_out("text_output=hello"), None);
        assert_eq!(parse_text_out("metrics[c0000].wall_ms=1.0"), None);
    }

    #[test]
    fn resolves_a_relative_reported_path_against_the_cwd() {
        // Observed live: `text_out[c0000]=out/seg_c0000.txt` when `--text-out`
        // is relative, so the raw string is not always usable as-is.
        let absolute = resolve_reported_path("/tmp/x/out/seg_c0000.txt");
        assert_eq!(absolute, PathBuf::from("/tmp/x/out/seg_c0000.txt"));

        let relative = resolve_reported_path("out/seg_c0000.txt");
        assert!(relative.is_absolute(), "{relative:?}");
        assert!(relative.ends_with("out/seg_c0000.txt"));
        // It must not be interpreted as a bare cwd-relative name.
        assert_ne!(relative, PathBuf::from("out/seg_c0000.txt"));
    }

    #[test]
    fn parses_metric_ids() {
        assert_eq!(
            parse_metric_id("metrics[c0001].wall_ms=62.6556"),
            Some("c0001".to_string())
        );
        assert_eq!(parse_metric_id("metrics[c0001].rtf=0.02"), Some("c0001".to_string()));
        assert_eq!(parse_metric_id("text_out[c0001]=/x"), None);
        assert_eq!(parse_metric_id("audiocpp_cli failed: nope"), None);
    }

    #[test]
    fn chunk_ids_are_padded_and_safe() {
        assert_eq!(chunk_id(0), "c0000");
        assert_eq!(chunk_id(42), "c0042");
        // Already `safe_output_name`-clean, so the CLI will not rewrite it.
        assert!(chunk_id(7)
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'));
    }

    #[test]
    fn default_text_out_path_matches_the_cli_convention() {
        let path = default_text_out_path(std::path::Path::new("/w/out/seg.txt"), "c0000");
        assert_eq!(path, PathBuf::from("/w/out/seg_c0000.txt"));
    }

    #[test]
    fn sequence_args_never_pass_request_option() {
        // PoC §4.1: `--request-option` + `--request-sequence` is rejected.
        let args = asr_args(
            std::path::Path::new("/w/requests.json"),
            std::path::Path::new("/w/out/seg.txt"),
            std::path::Path::new("/res/models/silero_vad"),
            "sense_asr",
        );

        assert!(args.iter().any(|a| a == "--request-sequence"));
        assert!(args.iter().any(|a| a == "--metrics"));
        assert!(
            !args.iter().any(|a| a.starts_with("--request-option")),
            "{args:?}"
        );
        // The VAD path must be spelled out; the CLI ships a relative default.
        assert!(args
            .iter()
            .any(|a| a == "sense_asr.vad_model_path=/res/models/silero_vad"));
    }

    /// Regression: the session-option prefix is family-scoped, so a plan that
    /// selected a different ASR family must not be handed
    /// `sense_asr.vad_model_path`. `qwen3_asr` has no such option at all.
    #[test]
    fn sequence_args_namespace_the_vad_option_by_family() {
        let qwen = asr_args(
            std::path::Path::new("/w/requests.json"),
            std::path::Path::new("/w/out/seg.txt"),
            std::path::Path::new("/res/models/silero_vad"),
            "qwen3_asr",
        );
        assert!(
            !qwen.iter().any(|a| a.contains("vad_model_path")),
            "qwen3_asr declares no vad_model_path; got {qwen:?}"
        );
        // The sequence/text plumbing must survive regardless of family.
        assert!(qwen.iter().any(|a| a == "--request-sequence"));
        assert!(qwen.iter().any(|a| a == "--metrics"));

        let fun = asr_args(
            std::path::Path::new("/w/requests.json"),
            std::path::Path::new("/w/out/seg.txt"),
            std::path::Path::new("/res/models/silero_vad"),
            "fun_asr_nano",
        );
        assert!(
            fun.iter()
                .any(|a| a == "fun_asr_nano.vad_model_path=/res/models/silero_vad"),
            "fun_asr_nano does declare it, under its own prefix; got {fun:?}"
        );
        assert!(!fun.iter().any(|a| a.starts_with("sense_asr.")));
    }

    #[test]
    fn requests_json_carries_per_request_options() {
        let dir = std::env::temp_dir().join(format!("monosub-req-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();

        let batch = vec![
            vad::VadChunk { index: 0, start_sample: 0, end_sample: 16000 },
            vad::VadChunk { index: 7, start_sample: 80000, end_sample: 240000 },
        ];
        let path = write_requests(&dir, &batch, "zh").expect("write requests.json");
        let written: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();

        let requests = written["requests"].as_array().expect("requests array");
        assert_eq!(requests.len(), 2);
        assert_eq!(requests[0]["id"], "c0000");
        // Relative to requests.json, which lives in `dir`.
        assert_eq!(requests[0]["audio"], "chunk_0000.wav");
        assert_eq!(requests[0]["language"], "zh");
        assert_eq!(requests[0]["options"]["audio_chunk_mode"], "none");
        assert_eq!(requests[1]["id"], "c0007");
        assert_eq!(requests[1]["audio"], "chunk_0007.wav");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn backend_failure_detection_matches_metal_errors_only() {
        assert!(is_backend_failure(&EngineError::Failed {
            message: "audiocpp_cli exited with code 1".to_string(),
            stderr_tail: "ggml_metal_init: failed to create command queue".to_string(),
        }));
        assert!(is_backend_failure(&EngineError::Failed {
            message: "unsupported backend".to_string(),
            stderr_tail: String::new(),
        }));
        // An unrelated failure must not trigger the CPU retry.
        assert!(!is_backend_failure(&EngineError::Failed {
            message: "could not open WAV input".to_string(),
            stderr_tail: "could not open WAV input: /tmp/x.wav".to_string(),
        }));
        assert!(!is_backend_failure(&EngineError::Protocol("bad json".to_string())));
    }

    #[test]
    fn temp_work_dir_is_removed_on_drop() {
        let path = {
            let dir = TempWorkDir::create("unit-drop").expect("create");
            std::fs::write(dir.path().join("scratch"), b"x").unwrap();
            assert!(dir.path().is_dir());
            dir.path().to_path_buf()
        };
        assert!(!path.exists(), "temp dir must be gone after drop");
    }

    #[test]
    fn temp_work_dir_replaces_a_stale_directory() {
        let first = TempWorkDir::create("unit-stale").expect("create");
        let path = first.path().to_path_buf();
        std::fs::write(path.join("leftover"), b"x").unwrap();
        // Leak the guard so the directory survives, simulating a crashed run.
        std::mem::forget(first);

        let second = TempWorkDir::create("unit-stale").expect("create over stale dir");
        assert_eq!(second.path(), path);
        assert!(!path.join("leftover").exists(), "stale files must be cleared");
    }
}

#[cfg(test)]
mod progress_monotonicity {
    use super::*;
    use std::sync::atomic::AtomicUsize;

    /// Progress must never regress, including across the Metal→CPU retry where a
    /// failed attempt may already have advanced it (review finding P2-1).
    #[test]
    fn reported_progress_is_strictly_monotonic() {
        let floor = AtomicUsize::new(0);
        // Climbing normally.
        for done in 1..=5 {
            assert!(should_report(&floor, done, 10), "{} should advance", done);
        }
        // The retry re-starts this batch's counter and can report a LOWER value;
        // that must be suppressed.
        assert!(!should_report(&floor, 0, 10));
        assert!(!should_report(&floor, 3, 10));
        assert!(!should_report(&floor, 5, 10), "equal value is not an advance");
        // ...and it must recover once the retry passes the old high-water mark.
        assert!(should_report(&floor, 6, 10));
    }

    /// A file whose chunks span several batches must climb across the boundary and
    /// land exactly on `total`, never exceeding it.
    #[test]
    fn progress_spans_batches_and_clamps_at_total() {
        let total = 300;
        // Batch 1 reports 1..=200.
        let floor = AtomicUsize::new(0);
        let mut emitted = Vec::new();
        for done in 1..=200 {
            if should_report(&floor, done, total) {
                emitted.push(done.min(total));
            }
        }
        assert_eq!(emitted.last(), Some(&200));
        // Batch 2 re-anchors on done_before=200.
        let floor = AtomicUsize::new(200);
        for done in 201..=300 {
            if should_report(&floor, done, total) {
                emitted.push(done.min(total));
            }
        }
        assert_eq!(emitted.first(), Some(&1));
        assert_eq!(emitted.last(), Some(&300), "must reach exactly total");
        assert!(emitted.windows(2).all(|w| w[0] < w[1]), "strictly increasing");
        // An over-count can never exceed `total`.
        assert!(!should_report(&floor, 999, total));
        assert_eq!(floor.load(std::sync::atomic::Ordering::SeqCst), total);
    }

    /// A zero-chunk (all-silence) run must not produce a bogus advance.
    #[test]
    fn zero_total_never_advances() {
        let floor = AtomicUsize::new(0);
        assert!(!should_report(&floor, 0, 0));
        assert!(!should_report(&floor, 5, 0));
    }
}
