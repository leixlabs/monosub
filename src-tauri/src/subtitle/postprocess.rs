//! Sentence splitting, proportional timestamp estimation and merging.
//!
//! Task: P1-10. Ported from `core-engine/monosub_core/transcriber.py`
//! (`_split_text_by_punctuation`, `_estimate_duration`, `_merge_short_segments`)
//! per docs/tech-spec/refactor-audiocpp.md §5.4.

use super::{PostProcessConfig, Segment};

/// Remove leftover `<|...|>` tags and trim; `None` means "drop this text".
pub fn clean_text(raw: &str) -> Option<String> {
    let _ = raw;
    unimplemented!("P1-10")
}

/// Split one chunk's text into cues by terminal punctuation, then by minor
/// punctuation, then hard-split anything still longer than `max_chars`.
pub fn split_sentences(text: &str, max_chars: usize) -> Vec<String> {
    let _ = (text, max_chars);
    unimplemented!("P1-10")
}

/// Effective character weight: CJK 1, latin word 1, digits 0.5, punctuation 0.
pub fn effective_len(text: &str) -> f64 {
    let _ = text;
    unimplemented!("P1-10")
}

/// Distribute `[start_s, end_s]` across `texts` proportionally to
/// [`effective_len`], guaranteeing monotonic non-overlapping cues.
pub fn distribute_times(
    texts: &[String],
    start_s: f64,
    end_s: f64,
    config: &PostProcessConfig,
) -> Vec<Segment> {
    let _ = (texts, start_s, end_s, config);
    unimplemented!("P1-10")
}

/// Merge adjacent cues separated by less than `gap_threshold` when the result
/// stays within `max_chars` / `max_duration`.
pub fn merge_short(segments: Vec<Segment>, config: &PostProcessConfig) -> Vec<Segment> {
    let _ = (segments, config);
    unimplemented!("P1-10")
}

/// Final pass: enforce `min_duration` and remove overlaps.
pub fn enforce_constraints(segments: Vec<Segment>, config: &PostProcessConfig) -> Vec<Segment> {
    let _ = (segments, config);
    unimplemented!("P1-10")
}
