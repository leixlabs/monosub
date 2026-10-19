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
fn short_speaker(text: &str, speaker: Option<&str>) -> String {    match speaker {
        Some(speaker) if !speaker.is_empty() => format!("{}: {}", speaker, text),
        _ => text.to_string(),
    }
}

/// Render the word-level sidecar consumed by video-editing agents.
///
/// This restores the contract the old Python engine shipped as `--raw`
/// (`transcriber.py::_build_scribe_words`), because a cutter needs spoken spans
/// **and** the silences between them:
///
/// ```json
/// {"words": [
///   {"type": "word", "text": "大", "start": 0.0, "end": 0.16, "speaker_id": null},
///   {"type": "spacing", "start": 0.16, "end": 0.4}
/// ]}
/// ```
///
/// `spacing` entries are emitted for every inter-word gap of at least
/// [`SPACING_MIN_SECONDS`], which is what an agent cuts on: a `spacing` span is a
/// stretch with no speech. The old implementation derived them from
/// `end != next_start`; identical rule, same field names, so existing
/// `pack_transcripts.py`-style consumers keep working.
///
pub fn render_word_sidecar(words: &[crate::engine::words::Word]) -> String {
    let mut out = String::from("{\n  \"words\": [\n");
    let mut first = true;

    let push_entry = |out: &mut String, first: &mut bool, entry: String| {
        if !*first {
            out.push_str(",\n");
        }
        out.push_str(&format!("    {}", entry));
        *first = false;
    };

    let mut previous_end: Option<f64> = None;
    for word in words {
        // A real silence opens a cuttable gap. Shorter gaps are coarticulation,
        // not silence, and would produce a spacing entry between every word.
        if let Some(previous_end) = previous_end {
            let gap = word.start - previous_end;
            if gap >= SPACING_MIN_SECONDS {
                push_entry(
                    &mut out,
                    &mut first,
                    format!(
                        "{{\"type\": \"spacing\", \"start\": {}, \"end\": {}}}",
                        format_number(previous_end),
                        format_number(word.start)
                    ),
                );
            }
        }

        push_entry(
            &mut out,
            &mut first,
            format!(
                "{{\"type\": \"word\", \"text\": \"{}\", \"start\": {}, \"end\": {}, \"speaker_id\": null}}",
                escape_json(&word.text),
                format_number(word.start),
                format_number(word.end)
            ),
        );
        previous_end = Some(word.end);
    }

    out.push_str("\n  ]\n}");
    out
}

/// Minimum inter-word gap that counts as a silence worth cutting on.
///
/// The old Python engine used `0.5 s`, but that came from Paraformer's timing,
/// and the Qwen3 forced aligner produces much tighter spans. Measured on
/// v0.9.1 over three real clips (`docs/tech-spec/word-timestamps-poc.md`), the
/// largest inter-word gaps were **0.40 s / 0.48 s / 0.40 s** — a 0.5 s threshold
/// emitted *zero* spacing entries, i.e. the sidecar carried no cut points at all.
///
/// `0.15` is the same value the cue splitter uses, so the sidecar and the
/// subtitles agree on what a pause is: an agent cutting at a `spacing` span gets
/// the same boundaries the subtitles show.
pub const SPACING_MIN_SECONDS: f64 = 0.15;

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

    /// The sidecar's contract for a video-editing agent: spoken spans plus the
    /// silences between them, in the same field names the old Python `--raw`
    /// mode used, so existing consumers keep working.
    #[test]
    fn word_sidecar_emits_words_and_spacings() {
        let words = vec![
            word(0.0, 0.16, "大"),
            word(0.16, 0.4, "家"),
            // 0.4 -> 1.2 is a 0.8 s silence: cuttable.
            word(1.2, 1.5, "好"),
        ];

        let json = render_word_sidecar(&words);

        assert!(json.contains(r#""type": "word""#));
        assert!(json.contains(r#""type": "spacing""#));
        assert!(json.contains(r#""text": "大""#));
        assert!(
            json.contains(r#"{"type": "spacing", "start": 0.4, "end": 1.2}"#),
            "the silence must be its own entry: {json}"
        );
        // The word entries cover every word exactly once.
        assert_eq!(json.matches(r#""type": "word""#).count(), 3);
        assert_eq!(json.matches(r#""type": "spacing""#).count(), 1);
    }

    /// Gaps shorter than the threshold are coarticulation, not silence: emitting
    /// a spacing between every word would give a cutter nothing to cut on.
    #[test]
    fn word_sidecar_omits_short_gaps() {
        // 0.05 s is well inside a single phrase.
        let words = vec![word(0.0, 0.3, "a"), word(0.35, 0.7, "b")];
        let json = render_word_sidecar(&words);

        assert!(!json.contains("spacing"), "0.05 s is not a silence: {json}");
    }

    /// The threshold must actually fire on measured data: the largest pause in a
    /// real clip was 0.48 s, and a 0.5 s rule produced no spacing at all.
    #[test]
    fn word_sidecar_emits_spacing_at_the_measured_boundary_value() {
        // A full-stop pause as measured (0.40 s).
        let words = vec![word(0.0, 0.4, "好"), word(0.8, 1.0, "我")];
        let json = render_word_sidecar(&words);

        assert!(
            json.contains("spacing"),
            "a measured sentence pause must become cuttable: {json}"
        );
    }

    /// `speaker_id` is present and null: consumers read the field whether or not
    /// diarization ran.
    #[test]
    fn word_sidecar_declares_the_null_speaker_field() {
        let json = render_word_sidecar(&[word(0.0, 0.1, "hi")]);
        assert!(json.contains(r#""speaker_id": null"#), "{json}");
    }

    /// A quote inside a word must not break the document.
    #[test]
    fn word_sidecar_escapes_quotes() {
        let json = render_word_sidecar(&[word(0.0, 0.1, "say \"hi\"")]);
        assert!(json.contains(r#"\"hi\""#), "{json}");
    }

    /// No words means no entries — the caller decides not to write the file.
    #[test]
    fn word_sidecar_with_no_words_is_empty() {
        let json = render_word_sidecar(&[]);
        assert!(json.contains(r#""words": ["#));
        assert!(!json.contains("\"type\""));
    }

    fn word(start: f64, end: f64, text: &str) -> crate::engine::words::Word {
        crate::engine::words::Word {
            index: 0,
            start,
            end,
            text: text.to_string(),
        }
    }

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

