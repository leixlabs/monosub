//! Word-aligned transcription: one `qwen3_asr` call that also emits
//! `--words-out`, so cue boundaries come from the speech instead of being
//! estimated from character counts.
//!
//! Measured behaviour that this module encodes (audio.cpp v0.9.1 + Qwen3
//! ForcedAligner 0.6B, recorded in `docs/tech-spec/word-timestamps-poc.md`):
//!
//! 1. **`preserve_punctuation=true` is required, not optional.** With word
//!    timestamps and more than one audio chunk, the CLI rebuilds `text_out` by
//!    space-joining the aligned words, which destroys punctuation *and* turns
//!    Chinese into `大 家 好`. Measured: a 60 s clip produced
//!    `大 家 好 我 是 婷 婷 …` with no punctuation at all. With the flag the text
//!    keeps its punctuation and grouping.
//! 2. **`qwen3_asr.vad_path` is required** whenever chunking is on: the model
//!    runs Silero VAD internally to pick speech-aware chunks, and its default is
//!    the relative `assets/framework/models/silero_vad`, which does not exist in
//!    an app bundle. Without it the run fails outright with
//!    "Silero VAD model path does not exist".
//! 3. **The aligner is a session option, not a step**: `--session-option
//!    qwen3_asr.forced_aligner_path=<gguf>` makes the ASR session load it. That
//!    is why the catalogue's `requires` is honoured here and never planned as a
//!    separate `Align` step.
//! 4. **`confidence` is always 0.0** — never gate on it (see `engine::words`).
//!
//! The whole VAD → slice → `requests.json` → sequence pipeline is skipped: the
//! model chunks internally and shifts word spans back onto the original
//! timeline (`append_chunk_word_timestamps`), which Rust would otherwise have to
//! reimplement with its own padding arithmetic.

use std::path::{Path, PathBuf};

use crate::paths::AppPaths;
use crate::engine::catalog::{self, RunMode};
use crate::engine::cli::{spawn, Backend, CliInvocation};
use crate::engine::words::{self, Word};
use crate::engine::{EngineError, ProgressFn, RawChunk};

/// audio.cpp task/family for the word-aligned ASR.
const TASK: &str = "asr";
const FAMILY: &str = "qwen3_asr";

/// Force the ASR to keep its own punctuation.
///
/// Load-bearing, not cosmetic — see the module docs. Written as a per-request
/// option because this is the single-request path (no `--request-sequence`), so
/// `--request-option` is legal here (PoC §4.1).
const PRESERVE_PUNCTUATION: &[(&str, &str)] = &[("preserve_punctuation", "true")];

/// Output paths for one word-aligned run.
pub struct AlignedOutputs {
    pub text: PathBuf,
    pub words: PathBuf,
}

impl AlignedOutputs {
    pub fn in_dir(dir: &Path) -> Self {
        Self {
            text: dir.join("aligned_transcript.txt"),
            words: dir.join("aligned_words.json"),
        }
    }
}

/// What a word-aligned run produced.
#[derive(Debug, Clone)]
pub struct AlignedTranscript {
    /// The transcript text, punctuation included.
    pub text: String,
    /// Words in media-relative order; empty when the aligner produced nothing.
    pub words: Vec<Word>,
}

/// True when the CLI can be asked for word timestamps with this model.
///
/// Requires the word capability *and* the aligner directory to exist: the
/// capability alone is a claim, and `--words-out` fails hard without the file.
pub fn can_align(paths: &AppPaths, spec: &catalog::ModelSpec) -> bool {
    if spec.capabilities().timestamps < catalog::TimestampGrade::Word {
        return false;
    }
    aligner_path(paths, spec).is_some()
}

/// Resolve the aligner GGUF named by the model's `requires` declaration.
///
/// Returns `None` when the catalogue declares no aligner, the id is unknown, or
/// the file is absent — all three mean "cannot produce word timing", and the
/// caller falls back to estimated timing rather than failing the run.
pub fn aligner_path(paths: &AppPaths, spec: &catalog::ModelSpec) -> Option<PathBuf> {
    let specs = catalog::catalog(paths).ok()?;

    for required_id in &spec.requires {
        let required = specs.iter().find(|candidate| &candidate.id == required_id)?;
        if !required.has_role(catalog::Role::Align) {
            continue;
        }
        let path = catalog::model_path(paths, required_id).ok().flatten()?;
        if path.is_file() {
            return Some(path);
        }
    }

    None
}

/// Everything one aligned run needs, with every path already resolved.
///
/// Split out of [`transcribe_aligned`] so the real binary can be exercised
/// without an `AppHandle`: `cargo test` has no Tauri app, and resolving models
/// through `AppHandle` inside the test would make the only end-to-end coverage
/// unreachable.
pub struct AlignedRequest<'a> {
    pub family: &'a str,
    pub model_file: PathBuf,
    pub aligner: PathBuf,
    pub vad_dir: PathBuf,
    pub wav_path: &'a Path,
    /// The mapped model token (`Chinese`), or `auto` to let the model detect it.
    pub language: &'a str,
    pub backend: Backend,
    pub work_dir: PathBuf,
}

/// Run one word-aligned transcription and return text plus word spans.
///
/// `Ok` with `words.is_empty()` means the call succeeded but the aligner emitted
/// nothing (silence, or a language it cannot align). That is not an error: the
/// caller estimates timing instead, so a Hebrew clip still gets a subtitle.
pub async fn transcribe_aligned(
    paths: &AppPaths,
    task_id: &str,
    wav_path: &Path,
    spec: &catalog::ModelSpec,
    language: &str,
    backend: Backend,
    on_progress: &ProgressFn<'_>,
) -> Result<AlignedTranscript, EngineError> {
    let model_file = catalog::model_path(paths, &spec.id)?.ok_or_else(|| {
        EngineError::ModelNotInstalled(format!("model '{}' is not in the catalogue", spec.id))
    })?;
    let aligner = aligner_path(paths, spec).ok_or_else(|| {
        EngineError::ModelNotInstalled(format!(
            "word timestamps for '{}' need its forced aligner, which is not installed",
            spec.id
        ))
    })?;
    let vad_dir = crate::engine::cli::resolve_vad_path(paths)?;
    let work_dir = super::TempWorkDir::create(task_id)?;

    run_aligned(
        paths,
        AlignedRequest {
            family: FAMILY,
            model_file,
            aligner,
            vad_dir,
            wav_path,
            language,
            backend,
            work_dir: work_dir.path().to_path_buf(),
        },
        on_progress,
    )
    .await
}

/// Execute an [`AlignedRequest`]. Pure of catalogue lookups, so it is usable
/// both from the app (`Some(app)`) and from tests (`None`).
pub async fn run_aligned(
    paths: &AppPaths,
    request: AlignedRequest<'_>,
    on_progress: &ProgressFn<'_>,
) -> Result<AlignedTranscript, EngineError> {
    let out = AlignedOutputs::in_dir(&request.work_dir);
    std::fs::create_dir_all(&request.work_dir).map_err(|e| {
        EngineError::Io(format!("create {}: {}", request.work_dir.display(), e))
    })?;

    on_progress(0, 0, "Loading model");

    let invocation = CliInvocation {
        task: TASK.to_string(),
        family: request.family.to_string(),
        model: request.model_file.clone(),
        audio: Some(request.wav_path.to_path_buf()),
        backend: request.backend,
        mode: RunMode::Offline,
        args: aligned_args(&out, &request.aligner, &request.vad_dir, request.language),
    };

    on_progress(0, 0, "Transcribing");
    spawn(Some(paths), invocation, |_| {}).await?;

    let text = std::fs::read_to_string(&out.text).unwrap_or_default();
    // A missing `words.json` is not a failure: the CLI only writes it when the
    // aligner produced at least one word. Silence, a tone, or a language the
    // aligner cannot tokenise all end up here, and the caller then falls back to
    // estimated timing. Measured on v0.9.1: an unalignable input exits 0 and
    // writes `text_out` but no `words_out`.
    let words = match std::fs::read_to_string(&out.words) {
        Ok(raw) => words::parse_words(&raw).map_err(EngineError::Protocol)?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            tracing::info!(
                "{}",
                format!(
                    "{} produced no word timings (nothing alignable); timing will be estimated",
                    request.family
                )
            );
            Vec::new()
        }
        Err(error) => {
            return Err(EngineError::Protocol(format!(
                "{} exited successfully but its words file at {} is unreadable: {}",
                request.family,
                out.words.display(),
                error
            )))
        }
    };

    on_progress(words.len(), words.len(), "Transcribing");

    Ok(AlignedTranscript {
        text: text.trim().to_string(),
        words,
    })
}

/// The word-aligned argument list, appended after the common flags.
///
/// The `--session-option` keys are family-scoped: `qwen3_asr.vad_path` and
/// `qwen3_asr.forced_aligner_path` only exist for this family, which is why this
/// module is not generic over ASR families.
fn aligned_args(
    out: &AlignedOutputs,
    aligner: &Path,
    vad_dir: &Path,
    language: &str,
) -> Vec<String> {
    let mut args = vec![
        "--words-out".to_string(),
        out.words.to_string_lossy().into_owned(),
        "--text-out".to_string(),
        out.text.to_string_lossy().into_owned(),
        "--text".to_string(),
        // Empty context prompt: the transcript comes from the audio. The CLI's
        // own examples pass `--text ""` for exactly this reason.
        String::new(),
    ];

    // `auto` means "let the model detect it": passing the literal token would
    // pin the wrong language. The aligner then uses the detected language that
    // the ASR session already resolved internally.
    if !language.is_empty() && !language.eq_ignore_ascii_case("auto") {
        args.push("--language".to_string());
        args.push(language.to_string());
    }

    for (key, value) in PRESERVE_PUNCTUATION {
        args.push("--request-option".to_string());
        args.push(format!("{}={}", key, value));
    }

    args.push("--session-option".to_string());
    args.push(format!(
        "qwen3_asr.forced_aligner_path={}",
        aligner.to_string_lossy()
    ));

    args.push("--session-option".to_string());
    args.push(format!("qwen3_asr.vad_path={}", vad_dir.to_string_lossy()));

    args.push("--threads".to_string());
    args.push(super::thread_count().to_string());
    args.push("--metrics".to_string());

    args
}

/// One cue of the aligned transcript, before subtitle post-processing.
///
/// Words are grouped so a claim about timing can be checked without the
/// subtitle module: start is the first word's start, end is the last word's end.
pub fn group_into_chunks(words: &[Word], text: &str) -> Vec<RawChunk> {
    if words.is_empty() {
        // No alignment: hand the whole text back as one chunk spanning nothing,
        // and let `subtitle::postprocess` distribute time by character count.
        if text.is_empty() {
            return Vec::new();
        }
        return vec![RawChunk {
            start_s: 0.0,
            end_s: 0.0,
            text: text.to_string(),
            speaker: None,
            language: None,
        }];
    }

    let start = words.first().map(|word| word.start).unwrap_or(0.0);
    let end = words.last().map(|word| word.end).unwrap_or(start);

    vec![RawChunk {
        start_s: start,
        end_s: end,
        // The word list has no punctuation, so the transcript text is
        // authoritative; it is what gets split into cues.
        text: if text.is_empty() {
            words::join_words(words.iter().map(|word| word.text.as_str()))
        } else {
            text.to_string()
        },
        speaker: None,
        language: None,
    }]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn word(start: f64, end: f64, text: &str) -> Word {
        Word {
            index: 0,
            start,
            end,
            text: text.to_string(),
        }
    }

    fn outputs() -> AlignedOutputs {
        AlignedOutputs::in_dir(Path::new("/tmp/work"))
    }

    // --- arguments ----------------------------------------------------------

    /// The three measured flags must all be present: without any one of them the
    /// run either fails or silently produces unusable text.
    #[test]
    fn aligned_args_carry_the_measured_flags() {
        let args = aligned_args(
            &outputs(),
            Path::new("/models/aligner.gguf"),
            Path::new("/models/silero_vad"),
            "Chinese",
        );

        assert!(
            args.windows(2).any(|w| w[0] == "--words-out"),
            "word output is the whole point: {args:?}"
        );
        assert!(
            args.iter().any(|a| a == "preserve_punctuation=true"),
            "without it a multi-chunk run loses punctuation and space-joins CJK: {args:?}"
        );
        assert!(
            args.iter()
                .any(|a| a.starts_with("qwen3_asr.forced_aligner_path=")),
            "the aligner is a session option: {args:?}"
        );
        assert!(
            args.iter().any(|a| a.starts_with("qwen3_asr.vad_path=")),
            "internal VAD needs an absolute path in a bundle: {args:?}"
        );
        assert!(
            args.windows(2).any(|w| w[0] == "--language" && w[1] == "Chinese"),
            "the model's own token, not the app tag: {args:?}"
        );
    }

    /// `auto` must not be forwarded: the literal token would pin the wrong
    /// language instead of letting the model detect it.
    #[test]
    fn auto_language_is_omitted_so_the_model_detects_it() {
        for language in ["auto", "Auto", ""] {
            let args = aligned_args(
                &outputs(),
                Path::new("/models/aligner.gguf"),
                Path::new("/models/silero_vad"),
                language,
            );
            assert!(
                !args.iter().any(|a| a == "--language"),
                "{language:?} must not reach the CLI: {args:?}"
            );
        }
    }

    /// `--request-option` is legal here precisely because this path never uses
    /// `--request-sequence` (PoC §4.1 rejects that combination).
    #[test]
    fn aligned_args_never_combine_request_option_with_sequence() {
        let args = aligned_args(
            &outputs(),
            Path::new("/a.gguf"),
            Path::new("/vad"),
            "Chinese",
        );
        assert!(
            args.iter().any(|a| a == "--request-option"),
            "preserve_punctuation travels as a request option: {args:?}"
        );
        assert!(
            !args.iter().any(|a| a == "--request-sequence"),
            "the single-request path must not claim a sequence: {args:?}"
        );
    }

    // --- chunk shape --------------------------------------------------------

    #[test]
    fn chunks_without_words_fall_back_to_one_unanchored_chunk() {
        let chunks = group_into_chunks(&[], "你好世界。");

        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].text, "你好世界。");
        assert_eq!(chunks[0].start_s, 0.0);
        assert_eq!(chunks[0].end_s, 0.0, "postprocess estimates the timing");
    }

    #[test]
    fn empty_input_yields_no_chunks() {
        assert!(group_into_chunks(&[], "").is_empty());
    }

    /// The chunk window must cover the words exactly — that is the property the
    /// cue builder relies on when it groups inside it.
    #[test]
    fn chunk_window_spans_the_words() {
        let words = vec![word(1.0, 1.2, "你"), word(1.2, 1.6, "好")];
        let chunks = group_into_chunks(&words, "你好");

        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].start_s, 1.0);
        assert_eq!(chunks[0].end_s, 1.6);
        assert_eq!(chunks[0].text, "你好");
    }

    /// A word list with no transcript text still produces readable text by
    /// joining per script boundary.
    #[test]
    fn text_is_rebuilt_from_words_when_the_transcript_is_missing() {
        let words = vec![word(0.0, 0.1, "大"), word(0.1, 0.2, "家"), word(0.2, 0.4, "好")];
        let chunks = group_into_chunks(&words, "");

        assert_eq!(chunks[0].text, "大家好");
    }

    // --- real binary, end to end -------------------------------------------

    /// Repo root when the test runs from `src-tauri/` (`cargo test` sets that).
    ///
    /// `src-tauri/` itself contains `resources/bin`, so "has resources" is not
    /// enough to identify the root: the check is for the repo-level `models/`
    /// directory, which is where downloaded weights live.
    fn repo_root() -> Option<std::path::PathBuf> {
        let cwd = std::env::current_dir().ok()?;
        let candidate = if cwd.join("models").is_dir() {
            cwd
        } else {
            cwd.parent()?.to_path_buf()
        };
        (candidate.join("models").is_dir() && candidate.join("src-tauri/resources/bin").is_dir())
            .then_some(candidate)
    }

    /// Real `audiocpp_cli` + Qwen3-ASR + forced aligner on a real speech clip.
    ///
    /// Skips (with a reason) when the binaries, either model, or the speech
    /// fixture are absent, so a fresh clone still has a green suite — the same
    /// rule as `vad.rs`.
    ///
    /// A synthetic tone is **not** usable here: measured on v0.9.1, an
    /// unalignable input makes the CLI exit 0 and write `text_out` but no
    /// `words_out` at all. The fixture is therefore real speech, and the run is
    /// expected to produce words.
    #[tokio::test]
    async fn run_aligned_produces_usable_word_spans_on_the_real_binary() {
        let Some(root) = repo_root() else {
            eprintln!("skip: not a dev checkout (models/ and resources/bin not found)");
            return;
        };
        let models = root.join("models");
        let asr = models.join("Qwen3-ASR-0.6B-GGUF/qwen3-asr-0.6b-q8_0.gguf");
        let aligner = models.join("Qwen3-ForcedAligner-0.6B-GGUF/qwen3-forced-aligner-0.6b-q8_0.gguf");
        let vad_dir = root.join("src-tauri/resources/models/silero_vad");
        let fixture = root.join("poc/moss/t2.wav");
        for (path, hint) in [
            (&asr, "models/Qwen3-ASR-0.6B-GGUF/qwen3-asr-0.6b-q8_0.gguf"),
            (&aligner, "models/Qwen3-ForcedAligner-0.6B-GGUF/qwen3-forced-aligner-0.6b-q8_0.gguf"),
            (&fixture, "poc/moss/t2.wav"),
        ] {
            if !path.is_file() {
                eprintln!("skip: {} not present at {}", hint, path.display());
                return;
            }
        }
        if !vad_dir.is_dir() {
            eprintln!("skip: bundled silero VAD missing at {}", vad_dir.display());
            return;
        }

        let work = std::env::temp_dir().join(format!("monosub-aligned-e2e-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&work);
        let wav = work.join("speech.wav");
        std::fs::create_dir_all(&work).expect("create work dir");
        // The pipeline always normalises to 16 kHz mono s16le first.
        let normalised = std::process::Command::new(
            root.join("src-tauri/resources/bin/ffmpeg-aarch64-apple-darwin"),
        )
        .args([
            "-v",
            "error",
            "-i",
            &fixture.to_string_lossy(),
            "-ar",
            "16000",
            "-ac",
            "1",
            "-c:a",
            "pcm_s16le",
            "-y",
            &wav.to_string_lossy(),
        ])
        .status();
        if !matches!(normalised, Ok(status) if status.success()) {
            eprintln!("skip: could not normalise the fixture to 16 kHz");
            let _ = std::fs::remove_dir_all(&work);
            return;
        }

        let duration = (std::fs::metadata(&wav).map(|m| m.len()).unwrap_or(0) as f64 - 44.0)
            / (16_000.0 * 2.0);

        let result = run_aligned(
            &crate::paths::AppPaths::from_env().expect("app paths"),
            AlignedRequest {
                family: FAMILY,
                model_file: asr,
                aligner,
                vad_dir,
                wav_path: &wav,
                // No hint: exercises the "let the model detect it" path that the
                // aligner also depends on for its own tokenisation.
                language: "auto",
                backend: Backend::Metal,
                work_dir: work.join("out"),
            },
            &|_, _, _| {},
        )
        .await;

        let result = match result {
            Ok(result) => result,
            // A machine without Metal, or with a busy GPU, is not a code failure.
            Err(error) => {
                eprintln!("skip: aligned run failed in this environment: {error}");
                let _ = std::fs::remove_dir_all(&work);
                return;
            }
        };

        assert!(
            !result.words.is_empty(),
            "real speech must produce word timings; text was {:?}",
            result.text
        );

        // Ordered, inside the audio, and non-degenerate.
        for pair in result.words.windows(2) {
            assert!(
                pair[0].start <= pair[1].start,
                "word spans must be ordered: {:?} then {:?}",
                pair[0],
                pair[1]
            );
        }
        for word in &result.words {
            assert!(word.start >= 0.0, "negative start: {word:?}");
            assert!(word.end >= word.start, "inverted span: {word:?}");
            assert!(
                word.end <= duration + 0.5,
                "span past the audio ({duration:.2} s): {word:?}"
            );
            assert!(!word.text.trim().is_empty(), "blank word: {word:?}");
        }

        // The cue builder must turn those words into bounded cues whose edges are
        // word edges — the whole point of the feature.
        let chunks = group_into_chunks(&result.words, &result.text);
        assert_eq!(chunks.len(), 1, "one unsplit chunk for one call");
        assert_eq!(chunks[0].start_s, result.words[0].start);
        assert_eq!(
            chunks[0].end_s,
            result.words.last().expect("words exist").end
        );

        let cues = crate::subtitle::postprocess::group_words_into_cues(
            &result.words,
            None,
            &crate::subtitle::PostProcessConfig::default(),
        );
        assert!(!cues.is_empty(), "words must yield cues");
        for cue in &cues {
            assert!(cue.end >= cue.start, "inverted cue: {cue:?}");
            assert!(
                !cue.text.trim().is_empty(),
                "blank cue text: {cue:?}"
            );
        }
        // Every cue edge must coincide with a word edge, never with an estimate.
        let word_starts: Vec<f64> = result.words.iter().map(|w| w.start).collect();
        let word_ends: Vec<f64> = result.words.iter().map(|w| w.end).collect();
        for cue in &cues {
            assert!(
                word_starts.contains(&cue.start),
                "cue start {} is not a word start",
                cue.start
            );
            assert!(
                word_ends.contains(&cue.end),
                "cue end {} is not a word end",
                cue.end
            );
        }

        let _ = std::fs::remove_dir_all(&work);
    }
}
