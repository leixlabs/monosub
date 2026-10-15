//! Silero VAD invocation and `vad_chunks.json` parsing.
//!
//! Task: P1-8. See docs/tech-spec/refactor-audiocpp.md §5.3 step 2 and PoC §4.3.
//!
//! Pinned baseline — keep these signatures stable for P1-9.

use std::path::{Path, PathBuf};

/// Tuning constants (spec §5.3): `max 15s` keeps per-chunk text short enough
/// for proportional timestamp estimation. PoC §4.3 flags `padding` as a
/// quality-sensitive knob to re-validate in P1-23.
pub const VAD_MAX_SECONDS: f64 = 15.0;
pub const VAD_MERGE_GAP_SECONDS: f64 = 0.5;
pub const VAD_PADDING_SECONDS: f64 = 0.25;

/// Sample rate audiocpp expects; WAV input must be 16 kHz mono s16le.
pub const SAMPLE_RATE: u32 = 16_000;

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

/// Run `--task vad --family silero_vad` and return the chunk plan.
///
/// An all-silent input yields `Ok(vec![])` with exit code 0 (PoC §4.3) —
/// callers must treat that as "write an empty subtitle", not an error.
pub async fn detect(
    app: Option<&tauri::AppHandle>,
    wav_path: &Path,
    work_dir: &Path,
) -> Result<Vec<VadChunk>, crate::engine::EngineError> {
    let _ = (app, wav_path, work_dir);
    unimplemented!("P1-8")
}

/// Parse a `vad_chunks.json` document.
pub fn parse_chunks(json: &str) -> Result<Vec<VadChunk>, crate::engine::EngineError> {
    unimplemented!("P1-8")
}

/// Output path used by [`detect`], kept public so callers can log it.
pub fn chunks_path(work_dir: &Path) -> PathBuf {
    work_dir.join("vad_chunks.json")
}
