//! Subtitle post-processing and multi-format writing (see docs/tech-spec/refactor-audiocpp.md §5.4).
//!
//! Owning tasks: P1-10..P1-11.
//!
//! Pinned baseline — `Segment` and `write_all` are the frozen interface used by
//! `commands/transcribe.rs` (P1-15).

pub mod postprocess;
pub mod writer;

use crate::engine::RawChunk;

/// One subtitle cue. Times are seconds; `speaker` is an already-formatted label
/// such as `Speaker A` (the `[Speaker A]: ` prefix is added by the writer).
#[derive(Debug, Clone, PartialEq)]
pub struct Segment {
    pub start: f64,
    pub end: f64,
    pub text: String,
    pub speaker: Option<String>,
}

/// Tuning constants ported from `core-engine/monosub_core/transcriber.py`
/// `MERGE_CONFIG` (spec §5.4).
pub struct PostProcessConfig {
    /// Minimum cue duration in seconds (Python `min_duration`).
    pub min_duration: f64,
    /// Maximum cue duration in seconds (Python `max_duration`).
    pub max_duration: f64,
    /// Maximum characters per cue (Python `max_chars`).
    pub max_chars: usize,
    /// Merge adjacent cues closer than this (Python `gap_threshold`).
    pub gap_threshold: f64,
}

impl Default for PostProcessConfig {
    fn default() -> Self {
        Self {
            min_duration: 0.3,
            max_duration: 10.0,
            max_chars: 50,
            gap_threshold: 0.5,
        }
    }
}

/// Clean → split by punctuation → distribute time by effective character count
/// → merge short neighbours → enforce min length and monotonicity.
///
/// Task: P1-10.
pub fn postprocess(chunks: &[RawChunk], config: &PostProcessConfig) -> Vec<Segment> {
    let _ = (chunks, config);
    unimplemented!("P1-10")
}

/// Supported output formats, mirroring `SubtitleWriter.write` in Python.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Srt,
    Vtt,
    Txt,
    Md,
    Json,
}

impl Format {
    pub fn parse(value: &str) -> Option<Format> {
        match value.trim().to_lowercase().as_str() {
            "srt" => Some(Format::Srt),
            "vtt" => Some(Format::Vtt),
            "txt" => Some(Format::Txt),
            "md" => Some(Format::Md),
            "json" => Some(Format::Json),
            _ => None,
        }
    }

    pub fn extension(self) -> &'static str {
        match self {
            Format::Srt => "srt",
            Format::Vtt => "vtt",
            Format::Txt => "txt",
            Format::Md => "md",
            Format::Json => "json",
        }
    }
}

/// `HH:MM:SS,mmm` / `HH:MM:SS.mmm` from integer milliseconds.
///
/// Task: P1-11. Both formats share this computation so the SRT/VTT outputs
/// cannot drift; `separator` is `,` or `.`.
pub fn format_timestamp(seconds: f64, separator: char) -> String {
    let _ = (seconds, separator);
    unimplemented!("P1-11")
}

/// Write one format. `output_path` is the exact destination file.
///
/// Task: P1-11.
pub fn write(
    segments: &[Segment],
    output_path: &std::path::Path,
    format: Format,
) -> Result<(), crate::engine::EngineError> {
    let _ = (segments, output_path, format);
    unimplemented!("P1-11")
}

/// Localized error when `json` is requested without word-level data, matching
/// the Python `ValueError` message.
pub const JSON_REQUIRES_RAW: &str =
    "JSON format requires raw word-level data. Use --raw with monosub to produce compatible output.";
