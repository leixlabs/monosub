//! SRT / VTT / TXT / MD / JSON writers.
//!
//! Task: P1-11. Ported from `core-engine/monosub_core/subtitle_writer.py`
//! (spec §5.4). Output must match the Python writer byte-for-byte for the same
//! segments; `src-tauri/tests/fixtures/` holds the golden files.

use super::Segment;
use std::path::Path;

/// `[Speaker A]: text` prefixing rule shared by SRT/VTT/TXT/MD.
pub fn with_speaker(text: &str, speaker: Option<&str>) -> String {
    let _ = (text, speaker);
    unimplemented!("P1-11")
}

/// SRT body (`1\nHH:MM:SS,mmm --> ...\n text \n\n`) without the file write.
pub fn render_srt(segments: &[Segment]) -> String {
    let _ = segments;
    unimplemented!("P1-11")
}

/// VTT body, including the `WEBVTT\n\n` header.
pub fn render_vtt(segments: &[Segment]) -> String {
    let _ = segments;
    unimplemented!("P1-11")
}

/// Plain text, one cue per line with the short `Speaker: text` label form.
pub fn render_txt(segments: &[Segment]) -> String {
    let _ = segments;
    unimplemented!("P1-11")
}

/// Markdown: `# <file stem>` title followed by blank-line separated cues.
pub fn render_md(segments: &[Segment], output_path: &Path) -> String {
    let _ = (segments, output_path);
    unimplemented!("P1-11")
}

/// JSON array of `{start, end, text, speaker?}` (spec §5.4).
pub fn render_json(segments: &[Segment]) -> String {
    let _ = segments;
    unimplemented!("P1-11")
}
