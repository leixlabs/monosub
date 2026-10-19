//! `--words-out` parsing: audio.cpp word-level timestamps.
//!
//! Word timing is what lets a cue boundary come from the speech itself instead
//! of being estimated in proportion to character count. Measured on audio.cpp
//! v0.9.1 with `qwen3_asr` + `qwen3_forced_aligner` (see
//! `docs/tech-spec/word-timestamps-poc.md`):
//!
//! - Times are **sample offsets** at the model's 16 kHz rate, so seconds are
//!   `sample / 16000`. Every span the CLI wrote was inside the audio duration.
//! - Chinese is split **per character** (`大`/`家`/`好`), matching the old
//!   Paraformer behaviour; space-separated languages split per word.
//! - `confidence` is **always 0.0** — the forced aligner emits no score, so it
//!   must never be used as a quality gate. Kept in the struct because it is in
//!   the payload, but nothing may branch on it.
//! - Punctuation is **absent** from the words (`text_out` has it). Cue
//!   boundaries therefore come from pauses, not from end-of-sentence marks.
//!
//! Shape (`app/workflow/file_sink.cpp::word_timestamps_to_json`):
//!
//! ```json
//! [{"start_sample":0,"end_sample":2560,"word":"大","confidence":0}]
//! ```

use serde::Deserialize;

/// Sample rate the pipeline guarantees before inference (16 kHz mono s16le).
const SAMPLE_RATE: f64 = 16_000.0;

/// One word (or, for CJK, one character) with its time span in seconds.
#[derive(Debug, Clone, PartialEq)]
pub struct Word {
    /// Index in the original sequence; kept for stable ordering after merges.
    pub index: usize,
    pub start: f64,
    pub end: f64,
    pub text: String,
}

impl Word {
    /// Duration in seconds, clamped at zero.
    ///
    /// Part of the public word API for consumers that need a span width (a
    /// caller rendering a waveform, a test asserting a span is not degenerate).
    /// The cue builder derives its own duration from first/last word instead, so
    /// this is not called from inside the app.
    #[allow(dead_code)]
    pub fn duration(&self) -> f64 {
        (self.end - self.start).max(0.0)
    }
}

/// `--words-out` as written by the CLI.
#[derive(Deserialize)]
struct WordWire {
    start_sample: i64,
    end_sample: i64,
    word: String,
    /// Present in the payload, always 0.0 in practice; deliberately unused.
    #[serde(default)]
    #[allow(dead_code)]
    confidence: Option<f32>,
}

/// Parse a `words.json` body into words ordered by start time.
///
/// An empty array is a valid answer (silence, or a language the aligner cannot
/// align) and yields an empty vector — the caller then falls back to estimated
/// timing rather than failing the run.
pub fn parse_words(json: &str) -> Result<Vec<Word>, String> {
    let raw: Vec<WordWire> =
        serde_json::from_str(json).map_err(|e| format!("invalid words.json: {}", e))?;

    let mut words: Vec<Word> = raw
        .into_iter()
        .enumerate()
        .map(|(index, wire)| {
            let start = wire.start_sample as f64 / SAMPLE_RATE;
            let end = wire.end_sample as f64 / SAMPLE_RATE;
            Word {
                index,
                start,
                // A zero-or-negative span carries no timing information; give it
                // the minimum width so downstream cue maths cannot divide by it.
                end: if end > start { end } else { start },
                text: wire.word.trim().to_string(),
            }
        })
        .filter(|word| !word.text.is_empty())
        .collect();

    // The aligner emits monotonically increasing spans, but a repair pass in
    // `fix_timestamp` can nudge individual ones; sorting here means the cue
    // builder never has to defend against it.
    words.sort_by(|a, b| {
        a.start
            .partial_cmp(&b.start)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.index.cmp(&b.index))
    });

    Ok(words)
}

/// Join words back into text, inserting a space only between space-separated
/// languages.
///
/// CJK words are single characters and must not be space-joined (`大 家 好`
/// would be wrong); Latin words must be (`The tribal chief`). The distinction is
/// made per adjacent pair, so mixed text works.
pub fn join_words<'a, I>(words: I) -> String
where
    I: IntoIterator<Item = &'a str>,
{
    let mut out = String::new();
    let mut previous_is_cjk = false;

    for word in words {
        let word = word.trim();
        if word.is_empty() {
            continue;
        }
        let starts_with_cjk = word.chars().next().map(is_cjk).unwrap_or(false);
        if !out.is_empty() && !previous_is_cjk && !starts_with_cjk {
            out.push(' ');
        }
        out.push_str(word);
        previous_is_cjk = word.chars().last().map(is_cjk).unwrap_or(false);
    }

    out
}

/// True for CJK ideographs and the kana/hangul blocks, which are written without
/// spaces. Fullwidth punctuation is deliberately excluded: it is punctuation, not
/// a word, and `parse_words` never yields it.
pub fn is_cjk(ch: char) -> bool {
    matches!(ch as u32,
        0x3040..=0x30FF       // Hiragana + Katakana
        | 0x3400..=0x4DBF     // CJK Extension A
        | 0x4E00..=0x9FFF     // CJK Unified Ideographs
        | 0xAC00..=0xD7AF     // Hangul syllables
        | 0xF900..=0xFAFF     // CJK Compatibility Ideographs
        | 0x20000..=0x2FA1F   // CJK Extensions B–F + Compatibility Supplement
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A real three-word slice as written by v0.9.1 for Chinese input: one
    /// character per entry, `confidence` 0, sample offsets at 16 kHz.
    const CJK_WORDS: &str = r#"[
      {"start_sample":0,"end_sample":2560,"word":"大","confidence":0},
      {"start_sample":2560,"end_sample":6400,"word":"家","confidence":0},
      {"start_sample":6400,"end_sample":12800,"word":"好","confidence":0}
    ]"#;

    #[test]
    fn parses_samples_into_seconds() {
        let words = parse_words(CJK_WORDS).expect("payload parses");

        assert_eq!(words.len(), 3);
        assert_eq!(words[0].text, "大");
        assert_eq!(words[0].start, 0.0);
        assert_eq!(words[0].end, 0.16);
        assert_eq!(words[2].start, 0.4);
        assert_eq!(words[2].end, 0.8);
    }

    /// An empty array is success (silence / unalignable language), not an error:
    /// the caller must be able to fall back to estimated timing.
    #[test]
    fn empty_array_is_success() {
        assert!(parse_words("[]").expect("empty array parses").is_empty());
    }

    #[test]
    fn rejects_malformed_payload() {
        let error = parse_words(r#"{"words":[]}"#).expect_err("an object is not the contract");
        assert!(error.contains("words.json"), "unhelpful error: {error}");
    }

    /// Zero-width spans are repaired rather than dropped: the word is real, only
    /// its timing is degenerate, and cue maths would divide by its duration.
    #[test]
    fn zero_width_spans_are_clamped_not_dropped() {
        let words = parse_words(
            r#"[{"start_sample":16000,"end_sample":16000,"word":"的","confidence":0}]"#,
        )
        .expect("payload parses");

        assert_eq!(words.len(), 1);
        assert_eq!(words[0].start, 1.0);
        assert_eq!(words[0].end, 1.0);
        assert_eq!(words[0].duration(), 0.0);
    }

    /// Blank entries are structural noise, not words.
    #[test]
    fn blank_words_are_dropped() {
        let words = parse_words(
            r#"[{"start_sample":0,"end_sample":100,"word":"  ","confidence":0},
                {"start_sample":100,"end_sample":200,"word":"好","confidence":0}]"#,
        )
        .expect("payload parses");

        assert_eq!(words.len(), 1);
        assert_eq!(words[0].text, "好");
    }

    /// Out-of-order spans are sorted, so the cue builder can assume monotonic
    /// input even after the upstream timestamp-repair pass.
    #[test]
    fn words_are_sorted_by_start() {
        let words = parse_words(
            r#"[{"start_sample":32000,"end_sample":33600,"word":"b","confidence":0},
                {"start_sample":16000,"end_sample":17600,"word":"a","confidence":0}]"#,
        )
        .expect("payload parses");

        assert_eq!(
            words.iter().map(|w| w.text.as_str()).collect::<Vec<_>>(),
            vec!["a", "b"]
        );
    }

    #[test]
    fn cjk_words_join_without_spaces() {
        assert_eq!(join_words(["大", "家", "好"]), "大家好");
    }

    #[test]
    fn latin_words_join_with_spaces() {
        assert_eq!(
            join_words(["The", "tribal", "chief"]),
            "The tribal chief"
        );
    }

    /// Mixed scripts: no space between CJK, but one on each side of a Latin run.
    #[test]
    fn mixed_scripts_join_per_boundary() {
        assert_eq!(join_words(["我", "用", "Mac"]), "我用Mac");
        assert_eq!(join_words(["Mac", "很", "快"]), "Mac很快");
    }

    #[test]
    fn is_cjk_covers_kana_and_hangul_but_not_punctuation() {
        assert!(is_cjk('한'));
        assert!(is_cjk('あ'));
        assert!(is_cjk('漢'));
        assert!(!is_cjk('A'));
        assert!(!is_cjk('，'));
        assert!(!is_cjk('。'));
    }
}
