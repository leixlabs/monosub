//! Silero VAD invocation and `vad_chunks.json` parsing.
//!
//! Task: P1-8. See docs/tech-spec/refactor-audiocpp.md §5.3 step 2 and PoC §4.3.
//!
//! Pinned baseline — keep these signatures stable for P1-9.

use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::paths::AppPaths;
use crate::engine::catalog::RunMode;
use crate::engine::cli::{spawn, Backend, CliInvocation};
use crate::engine::EngineError;

/// Tuning constants (spec §5.3): `max 15s` keeps per-chunk text short enough
/// for proportional timestamp estimation.
///
/// `padding` and `merge_gap` were re-validated in P1-23 and deliberately moved
/// off the spec's drafted `0.25` / `0.5`. `padding p` widens every speech span by
/// `p` on both sides, so a raw gap `g` becomes `g - 2p`, and the two spans merge
/// when `g - 2p < merge_gap`. At the drafted values that means any gap under
/// `1.0s` fuses two utterances into one chunk; measured on 0.8s-spaced speech the
/// drafted pair merged 4 sentences into 3 and misattributed a boundary token
/// (`会议纪要？` landed in the previous sentence's chunk). See
/// docs/tech-spec/audiocpp-accuracy-p1-23.md §3.3/§3.4.
///
/// The asymmetry is why this matters: over-merging loses a boundary for good,
/// whereas under-merging is recovered downstream, because `subtitle::postprocess`
/// re-joins adjacent cues closer than its own 0.5s `gap_threshold`. At `0.10` /
/// `0.2`, pauses up to ~0.4s still fuse inside the VAD pass and longer ones stay
/// separate. Re-confirm against real material before release (PoC §6.1).
pub const VAD_MAX_SECONDS: f64 = 15.0;
pub const VAD_MERGE_GAP_SECONDS: f64 = 0.2;
pub const VAD_PADDING_SECONDS: f64 = 0.1;

/// Sample rate audiocpp expects; WAV input must be 16 kHz mono s16le.
pub const SAMPLE_RATE: u32 = 16_000;

/// audio.cpp task for the VAD stage. The `family` is **not** a constant here:
/// several VAD families exist (`silero_vad`, `pulsevad`, `firered_vad`, …) and a
/// GGUF embeds its own, so passing the wrong one fails the same way an ASR
/// family mismatch does. The selected model supplies it via [`VadModel`].
const TASK: &str = "vad";

/// Bundled default VAD family: Silero, shipped in `resources/models/silero_vad`
/// as a `.safetensors` directory rather than a downloadable GGUF.
pub const DEFAULT_FAMILY: &str = "silero_vad";

/// What the VAD stage should run.
///
/// `model` is either the bundled Silero **directory** (families that take a
/// directory via `--model`) or a downloaded GGUF file. `--model` accepts both,
/// which is why one field covers it.
#[derive(Debug, Clone)]
pub struct VadModel {
    /// audio.cpp family, e.g. `silero_vad` or `pulsevad`.
    pub family: String,
    /// Directory (bundled) or GGUF file (downloaded).
    pub model: std::path::PathBuf,
}

/// Name of the bundled silero model; `cli::resolve_vad_path` validates it, so
/// this copy exists only for the tests that fabricate a resources tree.
#[cfg(test)]
const VAD_MODEL_FILE: &str = "silero_vad_16k.safetensors";

/// One VAD chunk in sample offsets, exactly as `--vad-chunks-out` reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VadChunk {
    pub index: usize,
    pub start_sample: u64,
    pub end_sample: u64,
}

impl VadChunk {
    pub fn start_s(&self) -> f64 {
        self.start_sample as f64 / SAMPLE_RATE as f64
    }

    pub fn end_s(&self) -> f64 {
        self.end_sample as f64 / SAMPLE_RATE as f64
    }
}

/// Wire format of one `--vad-chunks-out` element.
#[derive(Debug, Deserialize)]
struct RawVadChunk {
    index: usize,
    start_sample: u64,
    end_sample: u64,
}

/// Run `--task vad --family silero_vad` and return the chunk plan.
///
/// An all-silent input yields `Ok(vec![])` with exit code 0 (PoC §4.3) —
/// callers must treat that as "write an empty subtitle", not an error.
pub async fn detect(
    paths: &AppPaths,
    wav_path: &Path,
    work_dir: &Path,
    vad: &VadModel,
) -> Result<Vec<VadChunk>, crate::engine::EngineError> {
    let invocation = vad_invocation(vad, wav_path, work_dir);
    let out_path = chunks_path(work_dir);

    // The CLI only writes `--vad-chunks-out` if the directory already exists.
    std::fs::create_dir_all(work_dir).map_err(|e| {
        EngineError::Io(format!("create {}: {}", work_dir.display(), e))
    })?;

    spawn(Some(paths), invocation, |_| {}).await?;

    let raw = std::fs::read_to_string(&out_path).map_err(|e| {
        EngineError::Protocol(format!(
            "{} exited successfully but wrote no readable chunk plan at {}: {}",
            vad.family,
            out_path.display(),
            e
        ))
    })?;
    parse_chunks(&raw)
}

/// Parse a `vad_chunks.json` document.
pub fn parse_chunks(json: &str) -> Result<Vec<VadChunk>, crate::engine::EngineError> {
    let raw: Vec<RawVadChunk> = serde_json::from_str(json)
        .map_err(|e| EngineError::Protocol(format!("vad_chunks.json: {}", e)))?;

    raw.into_iter()
        .map(|c| {
            if c.end_sample < c.start_sample {
                return Err(EngineError::Protocol(format!(
                    "vad_chunks.json: chunk {} has end_sample {} before start_sample {}",
                    c.index, c.end_sample, c.start_sample
                )));
            }
            Ok(VadChunk {
                index: c.index,
                start_sample: c.start_sample,
                end_sample: c.end_sample,
            })
        })
        .collect()
}

/// Output path used by [`detect`], kept public so callers can log it.
pub fn chunks_path(work_dir: &Path) -> PathBuf {
    work_dir.join("vad_chunks.json")
}

/// Build the `--task vad` invocation from spec §5.3 step 2 (PoC §4.3).
///
/// `--task/--family/--model/--backend/--audio` are added by [`spawn`] from the
/// struct fields, so [`CliInvocation::args`] carries only the VAD-specific
/// flags (same convention as the P1-9 `--request-sequence` invocation).
fn vad_invocation(
    vad: &VadModel,
    wav_path: &Path,
    work_dir: &Path,
) -> CliInvocation {
    CliInvocation {
        task: TASK.to_string(),
        family: vad.family.clone(),
        model: vad.model.clone(),
        audio: Some(wav_path.to_path_buf()),
        backend: Backend::Cpu,
        mode: RunMode::Offline,
        args: vad_args(&chunks_path(work_dir)),
    }
}

/// The VAD-specific flags appended after the common ones.
fn vad_args(out_path: &Path) -> Vec<String> {
    [
        ("--vad-chunks-out", out_path.to_string_lossy().into_owned()),
        ("--vad-chunk-max-seconds", seconds_arg(VAD_MAX_SECONDS)),
        (
            "--vad-chunk-merge-gap-seconds",
            seconds_arg(VAD_MERGE_GAP_SECONDS),
        ),
        (
            "--vad-chunk-padding-seconds",
            seconds_arg(VAD_PADDING_SECONDS),
        ),
    ]
    .into_iter()
    .flat_map(|(flag, value)| [flag.to_string(), value])
    .collect()
}

/// Render a constant without a spurious `.0`/trailing-zero tail.
fn seconds_arg(value: f64) -> String {
    let mut s = format!("{:.2}", value);
    while s.contains('.') && s.ends_with('0') {
        s.pop();
    }
    if s.ends_with('.') {
        s.pop();
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    /// PoC §4.3: `padding=0.25` on a 6 s clip widened 11296..82912 to this.
    const POC_CHUNKS: &str = r#"[{"index":0,"start_sample":7296,"end_sample":86912}]"#;

    #[test]
    fn parse_chunks_reads_poc_sample() {
        assert_eq!(
            parse_chunks(POC_CHUNKS).expect("parse"),
            vec![VadChunk {
                index: 0,
                start_sample: 7296,
                end_sample: 86912,
            }]
        );
    }

    #[test]
    fn parse_chunks_empty_array_is_ok() {
        // All-silent input: exit code 0, zero chunks (PoC §4.3).
        assert_eq!(parse_chunks("[]").expect("parse"), Vec::new());
    }

    #[test]
    fn parse_chunks_reads_multiple_chunks_in_order() {
        let json = r#"[{"index":0,"start_sample":0,"end_sample":16000},
                       {"index":1,"start_sample":80000,"end_sample":240000}]"#;
        let chunks = parse_chunks(json).expect("parse");
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].start_s(), 0.0);
        assert_eq!(chunks[1].start_sample, 80000);
        assert_eq!(chunks[1].end_s(), 15.0);
    }

    #[test]
    fn parse_chunks_rejects_malformed_documents() {
        for json in [
            "",
            "null",
            "{}",
            "not json at all",
            r#"[{"index":0}]"#,
            r#"[{"start_sample":0,"end_sample":10}]"#,
            r#"[{"index":0,"start_sample":"0","end_sample":10}]"#,
            r#"[{"index":0,"start_sample":-1,"end_sample":10}]"#,
            r#"[{"index":0,"start_sample":10,"end_sample":5}]"#,
        ] {
            match parse_chunks(json) {
                Err(EngineError::Protocol(_)) => {}
                other => panic!("expected Protocol for {json:?}, got {other:?}"),
            }
        }
    }

    /// The bundled Silero VAD as a [`VadModel`], for the tests that exercise the
    /// real CLI.
    fn bundled_vad() -> VadModel {
        VadModel {
            family: DEFAULT_FAMILY.to_string(),
            model: crate::engine::cli::resolve_vad_path(&crate::paths::AppPaths::from_env().expect("paths")).expect("bundled silero vad dir"),
        }
    }

    #[test]
    fn vad_invocation_matches_the_spec_vector() {
        let invocation = vad_invocation(
            &VadModel {
                family: DEFAULT_FAMILY.to_string(),
                model: PathBuf::from("/res/models/silero_vad"),
            },
            Path::new("/tmp/work/audio.wav"),
            Path::new("/tmp/work"),
        );

        assert_eq!(invocation.task, "vad");
        assert_eq!(invocation.family, "silero_vad");
        assert_eq!(invocation.model, PathBuf::from("/res/models/silero_vad"));
        assert_eq!(invocation.backend, Backend::Cpu);
        assert_eq!(
            invocation.audio.as_deref(),
            Some(Path::new("/tmp/work/audio.wav"))
        );
        assert_eq!(
            invocation.args,
            vec![
                "--vad-chunks-out",
                "/tmp/work/vad_chunks.json",
                "--vad-chunk-max-seconds",
                "15",
                "--vad-chunk-merge-gap-seconds",
                "0.2",
                "--vad-chunk-padding-seconds",
                "0.1",
            ]
        );

        // Full argv, after `cli::spawn` prepends the common flags. Values come
        // from the constants, so tuning them cannot silently desync this test.
        let argv = full_argv(&invocation);
        for (flag, value) in [
            ("--task", "vad".to_string()),
            ("--family", "silero_vad".to_string()),
            ("--model", "/res/models/silero_vad".to_string()),
            ("--backend", "cpu".to_string()),
            ("--audio", "/tmp/work/audio.wav".to_string()),
            ("--vad-chunks-out", "/tmp/work/vad_chunks.json".to_string()),
            ("--vad-chunk-max-seconds", seconds_arg(VAD_MAX_SECONDS)),
            ("--vad-chunk-merge-gap-seconds", seconds_arg(VAD_MERGE_GAP_SECONDS)),
            ("--vad-chunk-padding-seconds", seconds_arg(VAD_PADDING_SECONDS)),
        ] {
            assert_eq!(arg_after(&argv, flag), Some(value.as_str()), "flag {flag}");
        }
        assert_eq!(argv.len(), 18, "no flags beyond the spec vector: {argv:?}");
    }

    /// The tuned VAD constants are a deliberate deviation from the spec's drafted
    /// values (P1-23); pin them so a revert is visible rather than silent.
    #[test]
    fn vad_constants_are_the_p1_23_tuned_values() {
        assert_eq!(VAD_MAX_SECONDS, 15.0);
        assert_eq!(VAD_MERGE_GAP_SECONDS, 0.2);
        assert_eq!(VAD_PADDING_SECONDS, 0.1);
        // Two spans merge when `gap - 2 * padding < merge_gap`.
        let merges_below = VAD_MERGE_GAP_SECONDS + 2.0 * VAD_PADDING_SECONDS;
        assert!(
            (merges_below - 0.4).abs() < 1e-9,
            "a {merges_below}s pause is the merge point; the drafted 0.25/0.5 values 
             merged everything under 1.0s"
        );
    }

    #[test]
    fn seconds_arg_has_no_trailing_zeros() {
        assert_eq!(seconds_arg(15.0), "15");
        assert_eq!(seconds_arg(0.5), "0.5");
        assert_eq!(seconds_arg(0.25), "0.25");
    }

    fn arg_after<'a>(argv: &'a [String], flag: &str) -> Option<&'a str> {
        let index = argv.iter().position(|arg| arg == flag)?;
        argv.get(index + 1).map(String::as_str)
    }

    /// The bundled CLI and silero model are gitignored, so tests that need them
    /// skip (with a reason) on machines where they are absent.
    struct BundledCli {
        cli: PathBuf,
        vad_dir: PathBuf,
    }

    fn bundled_cli() -> Option<BundledCli> {
        find_bundled_cli(&Path::new(env!("CARGO_MANIFEST_DIR")).join("resources"))
    }

    fn find_bundled_cli(resources: &Path) -> Option<BundledCli> {
        let vad_dir = resources.join("models").join("silero_vad");
        if !vad_dir.join(VAD_MODEL_FILE).is_file() {
            eprintln!("skip: silero VAD model missing at {}", vad_dir.display());
            return None;
        }

        let bin_dir = resources.join("bin");
        let cli = std::fs::read_dir(&bin_dir)
            .ok()
            .into_iter()
            .flatten()
            .flatten()
            .map(|entry| entry.path())
            .find(|path| {
                path.file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.starts_with("audiocpp_cli"))
            });
        match cli {
            Some(cli) => Some(BundledCli { cli, vad_dir }),
            None => {
                eprintln!("skip: audiocpp_cli missing under {}", bin_dir.display());
                None
            }
        }
    }

    #[test]
    fn find_bundled_cli_reports_a_skip_reason_when_files_are_absent() {
        let dir = test_dir("absent");
        let resources = dir.join("resources");
        std::fs::create_dir_all(resources.join("models").join("silero_vad"))
            .expect("create fake resources");

        // Model file missing.
        assert!(find_bundled_cli(&resources).is_none());

        // Model present, CLI missing.
        std::fs::write(
            resources
                .join("models")
                .join("silero_vad")
                .join(VAD_MODEL_FILE),
            b"stub",
        )
        .expect("write stub model");
        assert!(find_bundled_cli(&resources).is_none());

        // Both present.
        let bin_dir = resources.join("bin");
        std::fs::create_dir_all(&bin_dir).expect("create fake bin dir");
        let stub_cli = bin_dir.join("audiocpp_cli-test-triple");
        std::fs::write(&stub_cli, b"stub").expect("write stub cli");
        let found = find_bundled_cli(&resources).expect("both present");
        assert_eq!(found.cli, stub_cli);
        assert_eq!(found.vad_dir, resources.join("models").join("silero_vad"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Full argv for one invocation, mirroring `engine::cli`'s assembly order
    /// (pinned there by `argv_for_vad_invocation`).
    ///
    /// `cli::spawn` is not called directly because its dev-checkout resolver
    /// keys off the process cwd, which is `src-tauri/` under `cargo test`;
    /// everything after the argv — running the binary, reading the chunk file,
    /// parsing — is the real production path.
    fn full_argv(invocation: &CliInvocation) -> Vec<String> {
        let mut argv = vec![
            "--task".to_string(),
            invocation.task.clone(),
            "--family".to_string(),
            invocation.family.clone(),
            "--model".to_string(),
            invocation.model.to_string_lossy().into_owned(),
            "--backend".to_string(),
            match invocation.backend {
                Backend::Metal => "metal",
                Backend::Cpu => "cpu",
            }
            .to_string(),
        ];
        if let Some(audio) = &invocation.audio {
            argv.push("--audio".to_string());
            argv.push(audio.to_string_lossy().into_owned());
        }
        argv.extend(invocation.args.iter().cloned());
        argv
    }

    /// Run the real CLI with exactly the argv [`vad_invocation`] produces.
    fn run_cli(cli: &BundledCli, wav: &Path, work_dir: &Path) -> PathBuf {
        let invocation = vad_invocation(
            &VadModel {
                family: DEFAULT_FAMILY.to_string(),
                model: cli.vad_dir.clone(),
            },
            wav,
            work_dir,
        );
        let out_path = chunks_path(work_dir);
        let output = std::process::Command::new(&cli.cli)
            .args(full_argv(&invocation))
            .output()
            .expect("spawn bundled audiocpp_cli");
        assert!(
            output.status.success(),
            "vad exited with {:?}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        );
        out_path
    }

    fn test_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "monosub-vad-{}-{}",
            tag,
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&dir).expect("create test dir");
        dir
    }

    fn write_wav(path: &Path, samples: &[i16]) {
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: SAMPLE_RATE,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut writer = hound::WavWriter::create(path, spec).expect("create wav");
        for &s in samples {
            writer.write_sample(s).expect("write sample");
        }
        writer.finalize().expect("finalize wav");
    }

    /// Voiced-ish harmonic stack with 4 Hz amplitude modulation — what the
    /// bundled silero model flagged as speech during P1-8 verification.
    fn vowel_like(frames: usize) -> Vec<i16> {
        (0..frames)
            .map(|i| {
                let t = i as f64 / SAMPLE_RATE as f64;
                let mut v = 0.5 * (2.0 * std::f64::consts::PI * 120.0 * t).sin()
                    + 0.35 * (2.0 * std::f64::consts::PI * 240.0 * t).sin()
                    + 0.25 * (2.0 * std::f64::consts::PI * 700.0 * t).sin()
                    + 0.2 * (2.0 * std::f64::consts::PI * 1220.0 * t).sin()
                    + 0.15 * (2.0 * std::f64::consts::PI * 2600.0 * t).sin();
                v *= 0.9 * (0.5 + 0.5 * (2.0 * std::f64::consts::PI * 4.0 * t).sin());
                (v.clamp(-1.0, 1.0) * 32767.0).round() as i16
            })
            .collect()
    }

    #[test]
    fn real_cli_reports_no_chunks_for_silence() {
        let Some(cli) = bundled_cli() else {
            return;
        };
        let dir = test_dir("silence");
        let wav = dir.join("silence.wav");
        write_wav(&wav, &vec![0i16; 8 * SAMPLE_RATE as usize]);

        let out_path = run_cli(&cli, &wav, &dir);

        // PoC §4.3: all-silent input exits 0 with `[]`, i.e. `Ok(vec![])` —
        // the caller writes an empty subtitle rather than reporting an error.
        let parsed = parse_chunks(&std::fs::read_to_string(&out_path).expect("read chunks"))
            .expect("silence parses as Ok(vec![])");
        assert_eq!(parsed, Vec::new(), "silence must yield zero chunks");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn real_cli_finds_a_chunk_inside_a_synthetic_voiced_signal() {        let Some(cli) = bundled_cli() else {
            return;
        };
        let dir = test_dir("vowel");
        let wav = dir.join("vowel.wav");
        write_wav(&wav, &vowel_like(8 * SAMPLE_RATE as usize));

        let out_path = run_cli(&cli, &wav, &dir);

        let chunks = parse_chunks(&std::fs::read_to_string(&out_path).expect("read chunks"))
            .expect("parse chunks");
        assert!(!chunks.is_empty(), "synthetic voiced signal must be detected");

        let file_len = std::fs::metadata(&wav).expect("stat wav").len();
        for chunk in &chunks {
            assert!(chunk.end_sample >= chunk.start_sample);
            assert!(
                chunk.end_sample * 2 <= file_len,
                "chunk {} ends at sample {} (byte {}) past file size {}",
                chunk.index,
                chunk.end_sample,
                chunk.end_sample * 2,
                file_len
            );
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Full [`detect`] path against the real binary.
    ///
    /// `cli::resolve_vad_path` only finds the dev checkout when the process cwd
    /// is the repo root, and `cargo test` always sets the cwd to `src-tauri/`,
    /// so this branch cannot run under the standard command. The deterministic
    /// [`detect_reports_missing_resources_when_the_dev_checkout_is_unreachable`]
    /// covers that path instead, and [`run_cli`] covers the identical argv
    /// plus parsing.
    #[tokio::test]
    async fn detect_runs_the_bundled_cli_end_to_end() {
        if crate::engine::cli::resolve_vad_path(&crate::paths::AppPaths::from_env().expect("paths")).is_err() {
            eprintln!(
                "skip: dev resources not reachable from cwd {} (run from the repo root to enable)",
                std::env::current_dir()
                    .map(|d| d.display().to_string())
                    .unwrap_or_default()
            );
            return;
        }

        let dir = test_dir("detect");
        let wav = dir.join("vowel.wav");
        write_wav(&wav, &vowel_like(8 * SAMPLE_RATE as usize));

        let chunks = detect(&crate::paths::AppPaths::from_env().expect("paths"), &wav, &dir, &bundled_vad())
            .await
            .expect("detect on the bundled binary");
        assert!(!chunks.is_empty(), "synthetic voiced signal must be detected");
        assert!(chunks_path(&dir).is_file());

        // The same input has to stay `Ok(vec![])` when silent.
        let silent = dir.join("silence.wav");
        write_wav(&silent, &vec![0i16; 8 * SAMPLE_RATE as usize]);
        let empty = detect(&crate::paths::AppPaths::from_env().expect("paths"), &silent, &dir, &bundled_vad())
            .await
            .expect("silence is not an error");
        assert_eq!(empty, Vec::new());

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// `detect` surfaces an unresolvable bundled VAD dir as [`EngineError::Missing`]
    /// (never a panic), and does so before touching the audio file.
    #[tokio::test]
    async fn detect_reports_missing_resources_when_the_dev_checkout_is_unreachable() {
        if crate::engine::cli::resolve_vad_path(&crate::paths::AppPaths::from_env().expect("paths")).is_ok() {
            eprintln!("skip: dev resources are reachable from cwd, covered by the e2e test");
            return;
        }

        let dir = test_dir("missing");
        let missing_wav = dir.join("does-not-exist.wav");

        // Resolving the bundled dir is what fails here; `detect` must surface
        // that as a `Missing` error rather than panicking on an unresolvable
        // path. `resolve_vad_path` is the function under test.
        match crate::engine::cli::resolve_vad_path(&crate::paths::AppPaths::from_env().expect("paths")) {
            Err(EngineError::Missing(msg)) => {
                assert!(
                    msg.contains("silero_vad"),
                    "missing resource must name the VAD dir: {msg}"
                );
            }
            other => panic!("expected EngineError::Missing, got {other:?}"),
        }

        // And the audio file is never touched: the error is about the resource.
        assert!(!missing_wav.exists());

        let _ = std::fs::remove_dir_all(&dir);
    }
}
