//! Subtitle post-processing and multi-format writing (see docs/tech-spec/refactor-audiocpp.md §5.4).
//!
//! Owning tasks: P1-10..P1-11.
//!
//! Pinned baseline — `Segment` and `write_all` are the frozen interface used by
//! `commands/transcribe.rs` (P1-15).

pub mod postprocess;
pub mod writer;

use crate::engine::words::Word;
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
    /// Pause that splits a cue when timing comes from **words**.
    ///
    /// Deliberately separate from [`Self::gap_threshold`], which cannot be reused
    /// here: it is a *merge* threshold tuned for chunk-level windows, while this
    /// is a *split* threshold inside one continuous speech run. They measure
    /// different things and the measured values differ by more than 3x.
    ///
    /// Measured on audio.cpp v0.9.1 + Qwen3-ForcedAligner (`words.json` for a
    /// 10 s Chinese clip, `docs/tech-spec/word-timestamps-poc.md`): the pauses at
    /// real sentence boundaries were 0.40 s, 0.16 s and 0.16 s, while pauses
    /// *inside* a phrase were 0.08 s or 0.0 s. `0.15` is the value that separates
    /// those two populations on that data. With 0.5 s the same clip produced a
    /// single cue — word timing would have bought nothing.
    ///
    /// Coarticulation inside a phrase is at or below ~0.1 s, so this must stay
    /// above that or every character becomes its own cue.
    pub word_gap_threshold: f64,
}

impl Default for PostProcessConfig {
    fn default() -> Self {
        Self {
            min_duration: 0.3,
            max_duration: 10.0,
            max_chars: 50,
            gap_threshold: 0.5,
            word_gap_threshold: 0.15,
        }
    }
}

/// Build cues from the recognised chunks.
///
/// Two timing sources, chosen by what the engine actually produced:
///
/// - `words` non-empty (word-aligned inference ran): cue times come from the
///   words themselves via [`postprocess::group_words_into_cues`], and the chunk
///   windows are ignored — a cue is where the speech is, not where a VAD chunk
///   happened to end.
/// - `words` empty: the chunk window is shared out in proportion to character
///   count ([`postprocess::distribute_times`]), which is all a model without
///   word timing can support.
///
/// Both paths end in the same merge + constraint pass, so cue shape
/// (`min_duration`, `max_chars`, non-overlap) is defined once.
pub fn postprocess(
    chunks: &[RawChunk],
    words: &[Word],
    config: &PostProcessConfig,
) -> Vec<Segment> {
    // Speaker labels are per chunk on the estimated path and per run on the
    // aligned one. A word-aligned run with diarization is not offered yet, so
    // the aligned path takes the single label it was given; that is a documented
    // limitation, not an oversight (see docs/tech-spec/word-timestamps-poc.md).
    if !words.is_empty() {
        let speaker = chunks.first().and_then(|chunk| chunk.speaker.as_deref());
        let cues = postprocess::group_words_into_cues(words, speaker, config);
        let merged = postprocess::merge_short(cues, config);
        return postprocess::enforce_constraints(merged, config);
    }

    let mut segments: Vec<Segment> = Vec::new();

    for chunk in chunks {
        let Some(text) = postprocess::clean_text(&chunk.text) else {
            continue;
        };
        let sentences = postprocess::split_sentences(&text, config.max_chars);
        if sentences.is_empty() {
            continue;
        }
        // Each chunk already carries its own VAD window; distributing inside that
        // window preserves the model's timing information (spec §5.4 step 3).
        let start = chunk.start_s.max(0.0);
        let end = chunk.end_s.max(start);
        // The chunk's speaker label (when the plan produced one) is carried
        // through every cue split out of that chunk (design §6.5).
        segments.extend(postprocess::distribute_times(
            &sentences,
            start,
            end,
            chunk.speaker.as_deref(),
            config,
        ));
    }

    let merged = postprocess::merge_short(segments, config);
    postprocess::enforce_constraints(merged, config)
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
    // Spec §5.4: integer milliseconds via `(seconds * 1000.0).round()`. Negative
    // and non-finite inputs clamp to zero, matching the Python guard.
    if !seconds.is_finite() || seconds <= 0.0 {
        return format!("00:00:00{}000", separator);
    }
    let total_ms = (seconds * 1000.0).round() as u64;
    let millis = total_ms % 1000;
    let total_seconds = total_ms / 1000;
    let secs = total_seconds % 60;
    let minutes = (total_seconds / 60) % 60;
    let hours = total_seconds / 3600;
    format!(
        "{:02}:{:02}:{:02}{}{:03}",
        hours, minutes, secs, separator, millis
    )
}

/// Write one format. `output_path` is the exact destination file.
///
/// Task: P1-11.
pub fn write(
    segments: &[Segment],
    output_path: &std::path::Path,
    format: Format,
) -> Result<(), crate::engine::EngineError> {
    let content = match format {
        Format::Srt => writer::render_srt(segments),
        Format::Vtt => writer::render_vtt(segments),
        Format::Txt => writer::render_txt(segments),
        Format::Md => writer::render_md(segments, output_path),
        Format::Json => writer::render_json(segments),
    };

    std::fs::write(output_path, content)
        .map_err(|error| crate::engine::EngineError::Io(format!("{}: {}", output_path.display(), error)))
}

/// The first format writes to `output_path` verbatim, each later one replaces
/// only the extension (`os.path.splitext` semantics).
///
/// Task: U1-12. Moved here from `commands/transcribe.rs` because a second caller
/// now needs the same rule: the URL-import subtitle-hit path writes through the
/// same writers (url-import-design §5.4). Two copies of an output-naming rule is
/// how the two paths would quietly start naming files differently.
pub fn output_path_for(output_path: &std::path::Path, index: usize, format: Format) -> std::path::PathBuf {
    if index == 0 {
        return output_path.to_path_buf();
    }
    let mut path = output_path.to_path_buf();
    path.set_extension(format.extension());
    path
}

/// Parse a comma-separated `format` field: trim, lowercase, drop empties,
/// reject unknown values, deduplicate preserving order.
///
/// Shared so the URL-import path accepts exactly what the transcription path
/// accepts: a format list the user saved in Settings must mean the same thing on
/// both paths (url-import-design §5.4).
pub fn parse_formats(value: &str) -> Result<Vec<Format>, String> {
    let mut formats: Vec<Format> = Vec::new();
    for raw in value.split(',') {
        let token = raw.trim().to_lowercase();
        if token.is_empty() {
            continue;
        }
        let Some(format) = Format::parse(&token) else {
            return Err(format!("Unsupported subtitle format: {}", token));
        };
        if !formats.contains(&format) {
            formats.push(format);
        }
    }

    if formats.is_empty() {
        return Err("No valid subtitle format specified".to_string());
    }
    Ok(formats)
}

/// One path per requested format, derived from the primary output path.
pub fn output_paths_for(
    output_path: &std::path::Path,
    formats: &[Format],
) -> Vec<std::path::PathBuf> {
    formats
        .iter()
        .enumerate()
        .map(|(index, format)| output_path_for(output_path, index, *format))
        .collect()
}

/// Localized error when `json` is requested without word-level data, matching
/// the Python `ValueError` message.
///
/// Archived: `Format::Json` now renders cues (see `writer::render_json`), and the
/// word-level data lives in its own `<stem>.words.json` sidecar, so this message
/// describes a constraint the app no longer has. Kept only so the historical
/// wording is not lost; nothing reads it.
#[allow(dead_code)]
pub const JSON_REQUIRES_RAW: &str =
    "JSON format requires raw word-level data. Use --raw with monosub to produce compatible output.";
