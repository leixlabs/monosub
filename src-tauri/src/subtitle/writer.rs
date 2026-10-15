//! SRT / VTT / TXT / MD / JSON writers.
//!
//! Task: P1-11. Ported from `core-engine/monosub_core/subtitle_writer.py`
//! (spec §5.4). Output must match the Python writer byte-for-byte for the same
//! segments; `src-tauri/tests/fixtures/` holds the golden files.

use super::Segment;
use std::path::Path;

/// `[Speaker A]: text` prefixing rule shared by SRT/VTT/TXT/MD.
pub fn with_speaker(text: &str, speaker: Option<&str>) -> String {
    match speaker {
        Some(speaker) if !speaker.is_empty() => format!("[{}]: {}", speaker, text),
        _ => text.to_string(),
    }
}

/// SRT body (`1\nHH:MM:SS,mmm --> ...\n text \n\n`) without the file write.
pub fn render_srt(segments: &[Segment]) -> String {
    let mut out = String::new();
    for (index, segment) in segments.iter().enumerate() {
        let start = super::format_timestamp(segment.start, ',');
        let end = super::format_timestamp(segment.end, ',');
        let text = with_speaker(segment.text.trim(), segment.speaker.as_deref());
        out.push_str(&format!("{}\n", index + 1));
        out.push_str(&format!("{} --> {}\n", start, end));
        out.push_str(&format!("{}\n\n", text));
    }
    out
}

/// VTT body, including the `WEBVTT\n\n` header.
pub fn render_vtt(segments: &[Segment]) -> String {
    let mut out = String::from("WEBVTT\n\n");
    for (index, segment) in segments.iter().enumerate() {
        let start = super::format_timestamp(segment.start, '.');
        let end = super::format_timestamp(segment.end, '.');
        let text = with_speaker(segment.text.trim(), segment.speaker.as_deref());
        out.push_str(&format!("{}\n", index + 1));
        out.push_str(&format!("{} --> {}\n", start, end));
        out.push_str(&format!("{}\n\n", text));
    }
    out
}

/// Plain text, one cue per line with the short `Speaker: text` label form.
pub fn render_txt(segments: &[Segment]) -> String {
    let mut out = String::new();
    for segment in segments {
        out.push_str(&short_speaker(segment.text.trim(), segment.speaker.as_deref()));
        out.push('\n');
    }
    out
}

/// Markdown: `# <file stem>` title followed by blank-line separated cues.
pub fn render_md(segments: &[Segment], output_path: &Path) -> String {
    let stem = output_path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or_default();

    let mut out = format!("# {}\n\n", stem);
    for segment in segments {
        out.push_str(&short_speaker(segment.text.trim(), segment.speaker.as_deref()));
        out.push_str("\n\n");
    }
    out
}

/// JSON array of `{start, end, text, speaker?}` (spec §5.4).
pub fn render_json(segments: &[Segment]) -> String {
    if segments.is_empty() {
        return "[]".to_string();
    }

    let mut out = String::from("[\n");
    for (index, segment) in segments.iter().enumerate() {
        out.push_str("  {\n");
        out.push_str(&format!("    \"start\": {},\n", format_number(segment.start)));
        out.push_str(&format!("    \"end\": {},\n", format_number(segment.end)));
        out.push_str(&format!("    \"text\": \"{}\"", escape_json(&segment.text)));
        match segment.speaker.as_deref() {
            Some(speaker) => {
                out.push_str(&format!(",\n    \"speaker\": \"{}\"\n", escape_json(speaker)));
            }
            None => out.push('\n'),
        }
        if index + 1 < segments.len() {
            out.push_str("  },\n");
        } else {
            out.push_str("  }\n");
        }
    }
    out.push(']');
    out
}

/// Python's short speaker form used by TXT and MD: `Speaker A: text` (no brackets).
fn short_speaker(text: &str, speaker: Option<&str>) -> String {
    match speaker {
        Some(speaker) if !speaker.is_empty() => format!("{}: {}", speaker, text),
        _ => text.to_string(),
    }
}

/// Render an `f64` the way Python's `json.dumps` does for the value ranges that
/// appear in subtitles. Rust's `{:?}` matches CPython's float `repr` for normal
/// magnitudes; both only diverge for `|value| < 1e-4` or `>= 1e16` (exponent
/// formatting), which cannot occur in a seconds-based cue time.
fn format_number(value: f64) -> String {
    format!("{:?}", value)
}

/// Escape a string for a JSON string literal, mirroring `json.dumps`
/// (`ensure_ascii=False`): quote, backslash and the C0 control characters.
fn escape_json(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{0008}' => out.push_str("\\b"),
            '\u{000C}' => out.push_str("\\f"),
            ch if (ch as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", ch as u32)),
            ch => out.push(ch),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::subtitle::{write, Format, Segment};
    use std::path::PathBuf;

    const GOLDEN_SRT: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/segments.srt"));
    const GOLDEN_VTT: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/segments.vtt"));
    const GOLDEN_TXT: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/segments.txt"));
    const GOLDEN_MD: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/segments.md"));
    const GOLDEN_JSON: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/segments.json.golden"
    ));

    /// Mirrors `tests/fixtures/segments.json` (see `generate_goldens.py`).
    fn fixture_segments() -> Vec<Segment> {
        vec![
            Segment {
                start: 0.0,
                end: 2.5,
                text: "开放时间早上9点至下午5点。".to_string(),
                speaker: None,
            },
            Segment {
                start: 2.5,
                end: 6.0,
                text: "Yeah, u, I think people do that with almost everything in life.".to_string(),
                speaker: None,
            },
            Segment {
                start: 6.0,
                end: 9.5,
                text: "你好，世界。今天天气不错。".to_string(),
                speaker: Some("Speaker A".to_string()),
            },
            Segment {
                start: 9.5,
                end: 13.5,
                text: "真的吗？！太好了！——不过，等等；我们先确认一下。3.14 是圆周率！".to_string(),
                speaker: None,
            },
            Segment {
                start: 3600.0,
                end: 3618.0,
                text: "这是一条很长的字幕，用来验证写出器在处理超过一分钟时间轴与超长文本时是否稳定可靠。"
                    .to_string(),
                speaker: None,
            },
            Segment {
                start: 3618.0,
                end: 3620.0,
                text: "  前后带空格的文本 mixed with latin words 测试  ".to_string(),
                speaker: Some("Speaker B".to_string()),
            },
        ]
    }

    fn temp_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("monosub-writer-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("temp dir should be creatable");
        dir
    }

    #[test]
    fn format_timestamp_uses_integer_milliseconds() {
        assert_eq!(super::super::format_timestamp(0.0, ','), "00:00:00,000");
        assert_eq!(super::super::format_timestamp(2.5, ','), "00:00:02,500");
        assert_eq!(super::super::format_timestamp(3600.0, '.'), "01:00:00.000");
        assert_eq!(super::super::format_timestamp(3661.5, ','), "01:01:01,500");
        assert_eq!(super::super::format_timestamp(-1.0, ','), "00:00:00,000");
    }

    /// Documents the deliberate divergence from `SubtitleWriter.format_timestamp_srt`:
    /// Python truncates (`timedelta` → `int((ts % 1) * 1000)`), while spec §5.4
    /// mandates `(seconds * 1000.0).round()`. Cue times that are exact multiples of
    /// 1 ms (including every value produced by the fixture) are unaffected.
    #[test]
    fn timestamp_rounding_diverges_from_python_truncation() {
        assert_eq!(super::super::format_timestamp(0.2999, ','), "00:00:00,300");
        // Python's writer emits "00:00:00,299" here (truncation).
        assert_eq!(super::super::format_timestamp(1.9995, ','), "00:00:02,000");
        // Python's writer emits "00:00:01,999" here (no carry across seconds).
    }

    #[test]
    fn render_srt_matches_golden() {
        assert_eq!(render_srt(&fixture_segments()), GOLDEN_SRT);
    }

    #[test]
    fn render_vtt_matches_golden() {
        assert_eq!(render_vtt(&fixture_segments()), GOLDEN_VTT);
    }

    #[test]
    fn render_txt_matches_golden() {
        assert_eq!(render_txt(&fixture_segments()), GOLDEN_TXT);
    }

    #[test]
    fn render_md_matches_golden() {
        assert_eq!(
            render_md(&fixture_segments(), Path::new("/tmp/segments.md")),
            GOLDEN_MD
        );
    }

    #[test]
    fn render_json_matches_golden() {
        assert_eq!(render_json(&fixture_segments()), GOLDEN_JSON);
    }

    #[test]
    fn write_writes_golden_bytes_to_disk() {
        let segments = fixture_segments();
        let dir = temp_dir();
        let cases: [(Format, &str, &str); 5] = [
            (Format::Srt, "write-out.srt", GOLDEN_SRT),
            (Format::Vtt, "write-out.vtt", GOLDEN_VTT),
            (Format::Txt, "write-out.txt", GOLDEN_TXT),
            (Format::Md, "write-out.md", GOLDEN_MD),
            (Format::Json, "write-out.json", GOLDEN_JSON),
        ];

        for (format, name, expected) in cases {
            let path = dir.join(name);
            write(&segments, &path, format).expect("write should succeed");
            let written = std::fs::read_to_string(&path).expect("output should be readable");
            let _ = std::fs::remove_file(&path);
            // The MD title embeds the file stem, so compare against the golden
            // with its `# segments` title swapped for this file's stem.
            let expected = if format == Format::Md {
                expected.replacen("# segments", "# write-out", 1)
            } else {
                expected.to_string()
            };
            assert_eq!(written, expected, "format {name} mismatch");
        }
    }

    #[test]
    fn empty_segments_render_empty_outputs() {
        assert_eq!(render_srt(&[]), "");
        assert_eq!(render_vtt(&[]), "WEBVTT\n\n");
        assert_eq!(render_txt(&[]), "");
        assert_eq!(render_json(&[]), "[]");
    }

    #[test]
    fn with_speaker_prefixes_only_when_present() {
        assert_eq!(with_speaker("text", Some("Speaker A")), "[Speaker A]: text");
        assert_eq!(with_speaker("text", None), "text");
        assert_eq!(with_speaker("text", Some("")), "text");
    }
}

