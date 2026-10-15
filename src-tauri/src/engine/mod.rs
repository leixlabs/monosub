//! audio.cpp inference engine orchestration (see docs/tech-spec/refactor-audiocpp.md §4).
//!
//! Owning tasks: P1-5..P1-9.
//!
//! Pinned baseline — the exports below are the frozen interface between the
//! `engine` module (P1-5..P1-9) and `commands/transcribe.rs` (P1-15).

pub mod catalog;
pub mod cli;
pub mod vad;
pub mod wav;

pub use cli::{Backend, CliInvocation, CliOutput, EngineError};

/// One VAD segment or ASR transcript with absolute media-relative times.
#[derive(Debug, Clone, PartialEq)]
pub struct RawChunk {
    pub start_s: f64,
    pub end_s: f64,
    pub text: String,
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

/// Run the full pipeline for one file: VAD → slice → `requests.json` →
/// `--request-sequence` ASR → per-chunk text.
///
/// Temporary files live in `temp_dir()/monosub-<task_id>/` and are removed on
/// all exits (success, failure, cancel) via a `Drop` guard.
///
/// Tasks: P1-9 (orchestration), P1-8 (VAD), P1-7 (slicing).
pub async fn transcribe(
    app: &tauri::AppHandle,
    task_id: &str,
    wav_path: &std::path::Path,
    opts: &TranscribeOpts,
    on_progress: &ProgressFn<'_>,
) -> Result<Vec<RawChunk>, EngineError> {
    let _ = (app, task_id, wav_path, opts, on_progress);
    unimplemented!("P1-9")
}
