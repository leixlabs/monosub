//! SRT text → `Vec<subtitle::Segment>`.
//!
//! Task: U1-8. See docs/tech-spec/url-import-design.md §5.4.
//!
//! Platform subtitles are already cue-split by the uploader, so this is a pure
//! format conversion and **not** a post-processing pass: the timeline is kept
//! exactly as the platform wrote it. Running `subtitle::postprocess` here would
//! re-split at 50 chars / 10 s and destroy the author's cue boundaries.
//!
//! The parser is deliberately forgiving about everything that does not change
//! the meaning — a BOM, CRLF, a missing trailing newline, a non-numeric sequence
//! number, empty cue bodies — because real Bilibili tracks carry all of those.

use crate::subtitle::Segment;

/// Parse SRT text. An empty or unparseable document yields `[]`, which the
/// caller treats as "no subtitle" and falls back to transcription.
pub fn parse(text: &str) -> Vec<Segment> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let normalized = text.replace("\r\n", "\n").replace('\r', "\n");

    let mut segments: Vec<Segment> = Vec::new();
    // Accumulate one block at a time: a block is everything between blank lines.
    let mut block: Vec<&str> = Vec::new();

    for line in normalized.lines() {
        if line.trim().is_empty() {
            if !block.is_empty() {
                if let Some(segment) = parse_block(&block) {
                    segments.push(segment);
                }
                block.clear();
            }
            continue;
        }
        block.push(line);
    }
    if !block.is_empty() {
        if let Some(segment) = parse_block(&block) {
            segments.push(segment);
        }
    }

    segments
}

/// One block: an optional sequence number, one timing line, then body lines.
fn parse_block(lines: &[&str]) -> Option<Segment> {
    // The timing line is the first line that contains `-->`. Everything before it
    // is the sequence number (or a stray blank), which we do not need: the writer
    // renumbers from 1 anyway.
    let timing_index = lines
        .iter()
        .position(|line| line.contains("-->"))
        .unwrap_or(0);
    let (start, end) = parse_timing(lines.get(timing_index)?)?;

    let body = lines.get(timing_index + 1..).unwrap_or_default().join("\n");
    let body = body.trim();
    if body.is_empty() {
        // An empty cue carries no text; keeping it would emit a blank subtitle
        // line the user cannot act on.
        return None;
    }

    Some(Segment {
        start,
        end: end.max(start),
        text: body.to_string(),
        speaker: None,
    })
}

/// `HH:MM:SS,mmm --> HH:MM:SS,mmm`, tolerating a `.` separator, a 1-2 digit
/// hour field, and trailing cue settings (VTT-style `align:start`).
fn parse_timing(line: &str) -> Option<(f64, f64)> {
    let (left, right) = line.split_once("-->")?;
    let start = parse_timestamp(left)?;
    let end = parse_timestamp(right)?;
    Some((start, end))
}

/// One timestamp. Extra whitespace and a trailing settings blob are ignored.
fn parse_timestamp(raw: &str) -> Option<f64> {
    // Cue settings after the timestamp (`align:start position:0%`) are VTT-ish;
    // split on whitespace and take the first token that looks like a time.
    let token = raw
        .split_whitespace()
        .find(|token| token.contains(':') || token.contains(','))
        .unwrap_or(raw)
        .trim();

    let normalized = token.replace(',', ".");
    let parts: Vec<&str> = normalized.split(':').collect();
    let (hours, minutes, seconds) = match parts.as_slice() {
        [h, m, s] => (h.parse::<f64>().ok()?, m.parse::<f64>().ok()?, s.parse::<f64>().ok()?),
        [m, s] => (0.0, m.parse::<f64>().ok()?, s.parse::<f64>().ok()?),
        _ => return None,
    };

    // Subtitle timestamps are not sexagesimal beyond 60, but a malformed one
    // should still produce a number rather than silently wrapping.
    let total = hours * 3600.0 + minutes * 60.0 + seconds;
    total.is_finite().then_some(total.max(0.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_standard_block() {
        let srt = "1\n00:00:18,498 --> 00:00:21,925\n我們對愛情並不陌生\n\n\
                   2\n00:00:23,025 --> 00:00:26,975\n你知道規則我也一樣\n";
        let segments = parse(srt);

        assert_eq!(segments.len(), 2);
        assert!((segments[0].start - 18.498).abs() < 1e-6, "{:?}", segments[0]);
        assert!((segments[0].end - 21.925).abs() < 1e-6);
        assert_eq!(segments[0].text, "我們對愛情並不陌生");
        assert!((segments[1].start - 23.025).abs() < 1e-6);
        assert_eq!(segments[1].text, "你知道規則我也一樣");
        assert_eq!(segments[0].speaker, None);
    }

    #[test]
    fn keeps_multi_line_bodies() {
        let srt = "1\n00:00:01,000 --> 00:00:04,000\nfirst line\nsecond line\n\n";
        let segments = parse(srt);
        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].text, "first line\nsecond line");
    }

    #[test]
    fn accepts_a_missing_trailing_blank_line() {
        let srt = "1\n00:00:01,000 --> 00:00:02,000\nonly cue";
        assert_eq!(parse(srt).len(), 1);
        assert_eq!(parse(srt)[0].text, "only cue");
    }

    #[test]
    fn strips_a_byte_order_mark() {
        let srt = "\u{feff}1\n00:00:01,000 --> 00:00:02,000\nwith bom\n";
        let segments = parse(srt);
        assert_eq!(segments.len(), 1);
        // The BOM must not leak into the numeric field parsed below it either.
        assert_eq!(segments[0].text, "with bom");
        assert!((segments[0].start - 1.0).abs() < 1e-6);
    }

    #[test]
    fn accepts_crlf_line_endings() {
        let srt = "1\r\n00:00:01,000 --> 00:00:02,000\r\nwindows\r\n\r\n2\r\n00:00:03,000 --> 00:00:04,000\r\nsecond\r\n";
        let segments = parse(srt);
        assert_eq!(segments.len(), 2);
        assert_eq!(segments[0].text, "windows");
        assert_eq!(segments[1].text, "second");
        // No stray `\r` inside the text.
        assert!(!segments[0].text.contains('\r'));
    }

    #[test]
    fn empty_input_yields_nothing() {
        assert!(parse("").is_empty());
        assert!(parse("   \n\n\t\n").is_empty());
    }

    #[test]
    fn parses_timestamps_past_one_hour() {
        let srt = "1\n01:02:03,004 --> 02:00:00,000\nlong video\n";
        let segments = parse(srt);
        assert_eq!(segments.len(), 1);
        assert!((segments[0].start - (3723.004)).abs() < 1e-6, "{:?}", segments[0]);
        assert!((segments[0].end - 7200.0).abs() < 1e-6);
    }

    #[test]
    fn tolerates_a_non_strict_sequence_number() {
        // No number at all, an out-of-order number, and a junk number.
        for srt in [
            "00:00:01,000 --> 00:00:02,000\nno number\n",
            "42\n00:00:01,000 --> 00:00:02,000\nout of order\n",
            "abc\n00:00:01,000 --> 00:00:02,000\njunk number\n",
        ] {
            let segments = parse(srt);
            assert_eq!(segments.len(), 1, "{srt}");
            assert!(!segments[0].text.contains("-->"), "{srt}");
        }
    }

    #[test]
    fn skips_a_block_with_an_empty_body() {
        let srt = "1\n00:00:01,000 --> 00:00:02,000\n\n\
                   2\n00:00:03,000 --> 00:00:04,000\nreal\n\n\
                   3\n00:00:05,000 --> 00:00:06,000\n   \n";
        let segments = parse(srt);
        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].text, "real");
    }

    #[test]
    fn tolerates_a_dot_separator_and_cue_settings() {
        // Some extractors hand back VTT-flavoured timestamps.
        let srt = "1\n00:00:01.500 --> 00:00:02.750 align:start\nvtt-ish\n";
        let segments = parse(srt);
        assert_eq!(segments.len(), 1);
        assert!((segments[0].start - 1.5).abs() < 1e-6, "{:?}", segments[0]);
        assert!((segments[0].end - 2.75).abs() < 1e-6);
        assert_eq!(segments[0].text, "vtt-ish");
    }

    #[test]
    fn an_end_before_start_collapses_to_a_zero_length_cue() {
        let srt = "1\n00:00:05,000 --> 00:00:01,000\nbackwards\n";
        let segments = parse(srt);
        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].start, segments[0].end);
    }

    #[test]
    fn garbage_blocks_are_dropped_rather_than_panicking() {
        let srt = "1\nnot a timing line\nbody\n\n2\n00:00:01,000 --> 00:00:02,000\nok\n";
        let segments = parse(srt);
        // The first block has no `-->`, so `parse_timing` fails and it is
        // skipped — but the second still parses.
        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].text, "ok");
    }

    /// A footnote about the writer contract: an empty document must still be
    /// writable, so callers get `[]` rather than an error.
    #[test]
    fn empty_segments_are_still_writable() {
        let dir = std::env::temp_dir().join(format!("monosub-srt-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("empty.srt");
        crate::subtitle::write(&parse(""), &path, crate::subtitle::Format::Srt).unwrap();
        assert!(path.is_file());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
