//! Sentence splitting, proportional timestamp estimation and merging.
//!
//! Task: P1-10. Ported from `core-engine/monosub_core/transcriber.py`
//! (`_split_text_by_punctuation`, `_estimate_duration`, `_merge_short_segments`)
//! per docs/tech-spec/refactor-audiocpp.md §5.4.
//!
//! Two timing sources live here and they must not be mixed:
//!
//! - [`distribute_times`] *estimates* cue times by sharing a chunk's window out
//!   in proportion to character count. That is the only option when the model
//!   emits no word timing (SenseVoice, MOSS) and it is what makes a
//!   chunk boundary the only real timing anchor.
//! - [`group_words_into_cues`] *uses* word timing: a cue starts at its first
//!   word and ends at its last. No estimation is involved, which is why the old
//!   Python engine's paraphrase of this (`_split_by_punctuation`, driven by
//!   Paraformer's per-character timestamps) landed cues on the speech.
//!
//! Both funnel into the same [`merge_short`] / [`enforce_constraints`] pass, so
//! cue shape (minimum duration, non-overlap, `max_chars`) is defined once.

use super::{PostProcessConfig, Segment};
use crate::engine::words::Word;

/// Python `_merge_short_segments` also merges any cue shorter than 5 characters
/// (hard-coded next to `MERGE_CONFIG`), in addition to `min_duration`.
const MIN_MERGE_CHARS: usize = 5;

/// Language tokens `sense_asr` can put in a `<|…|>` tag.
///
/// Deliberately a closed set (PoC §4.2: the CLI accepts 23 tags but only
/// `{auto, zh, en, yue, ja, ko, nospeech}` map to a real token, and `auto` is
/// not an output tag). A broader "any short tag" rule cannot be used: the same
/// delimiters carry emotion (`<|NEUTRAL|>`), event (`<|Speech|>`) and ITN
/// markers, none of which is a language. A tag outside this set therefore means
/// *unknown language*, which downstream is the "translate anyway" branch — never
/// a guess and never an error.
const LANGUAGE_TAGS: &[&str] = &["zh", "en", "yue", "ja", "ko"];

/// Extract the ASR's own language tag (`<|zh|>`) from a raw transcript.
///
/// `clean_text` deletes every `<|…|>` tag, so the language has to be read *here,
/// before cleaning*, or it is lost for good (design §5). MOSS emits no such tag,
/// which is why the result is an `Option` rather than a
/// string.
pub fn extract_language_tag(raw: &str) -> Option<String> {
    let mut rest = raw;
    while let Some(open) = rest.find("<|") {
        let after_open = &rest[open + 2..];
        // A lone `<|` with no closing `|>` carries no tag.
        let close = after_open.find("|>")?;
        let tag = &after_open[..close];
        if !tag.contains('|') {
            let lowered = tag.to_lowercase();
            if LANGUAGE_TAGS.contains(&lowered.as_str()) {
                return Some(lowered);
            }
        }
        rest = &after_open[close + 2..];
    }

    None
}

/// Fold the per-chunk language tags into the recording's dominant language.
///
/// Majority wins; a tie breaks to the tag seen first, so the same input always
/// yields the same answer. `None` when no chunk carried a tag: the moss /
/// MOSS emits none, and "unknown" is the documented signal to
/// translate rather than to skip (design §5).
pub fn dominant_language<'a>(tags: impl IntoIterator<Item = Option<&'a str>>) -> Option<String> {
    // A Vec rather than a map: first-seen order is what breaks a tie.
    let mut counts: Vec<(String, usize)> = Vec::new();

    for tag in tags.into_iter().flatten() {
        let tag = tag.trim().to_lowercase();
        if tag.is_empty() {
            continue;
        }
        match counts.iter_mut().find(|(known, _)| *known == tag) {
            Some((_, count)) => *count += 1,
            None => counts.push((tag, 1)),
        }
    }

    // Explicit comparison: `max_by_key` returns the LAST maximum, which would
    // silently invert the first-seen tie-break.
    counts
        .into_iter()
        .fold(None, |best: Option<(String, usize)>, entry| match best {
            Some((best_tag, best_count)) if best_count >= entry.1 => Some((best_tag, best_count)),
            _ => Some(entry),
        })
        .map(|(tag, _)| tag)
}

/// Remove leftover `<|...|>` tags and trim; `None` means "drop this text".
pub fn clean_text(raw: &str) -> Option<String> {
    // Equivalent to Python's `re.sub(r'<\|[^|]+\|>', '', text).strip()`: a tag is
    // `<|` + one or more non-`|` characters + `|>`.
    let mut cleaned = String::with_capacity(raw.len());
    let mut rest = raw;
    while let Some(open) = rest.find("<|") {
        cleaned.push_str(&rest[..open]);
        let after_open = &rest[open + 2..];
        match after_open.find("|>") {
            Some(close) if !after_open[..close].is_empty() && !after_open[..close].contains('|') => {
                rest = &after_open[close + 2..];
            }
            _ => {
                cleaned.push_str("<|");
                rest = after_open;
            }
        }
    }
    cleaned.push_str(rest);

    let trimmed = cleaned.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

/// Split one chunk's text into cues by terminal punctuation, then by minor
/// punctuation, then hard-split anything still longer than `max_chars`.
pub fn split_sentences(text: &str, max_chars: usize) -> Vec<String> {
    let max_chars = max_chars.max(1);
    let mut out = Vec::new();

    for sentence in split_on_terminals(text) {
        if char_count(&sentence) <= max_chars {
            out.push(sentence);
            continue;
        }
        for part in split_on_minor(&sentence) {
            if char_count(&part) <= max_chars {
                out.push(part);
            } else {
                out.extend(hard_split(&part, max_chars));
            }
        }
    }

    out
}

/// Effective character weight: CJK 1, latin word 1, digits 0.5, punctuation 0.
pub fn effective_len(text: &str) -> f64 {
    let mut total = 0.0;
    let mut in_word = false;
    for ch in text.chars() {
        if is_cjk(ch) {
            total += 1.0;
            in_word = false;
        } else if ch.is_ascii_digit() {
            total += 0.5;
            in_word = false;
        } else if ch.is_alphabetic() {
            // A run of letters (incl. accented latin) is one word of weight 1.
            if !in_word {
                total += 1.0;
                in_word = true;
            }
        } else {
            // Whitespace and punctuation carry no reading time and end a word.
            in_word = false;
        }
    }
    total
}

/// Distribute `[start_s, end_s]` across `texts` proportionally to
/// [`effective_len`], guaranteeing monotonic non-overlapping cues.
pub fn distribute_times(
    texts: &[String],
    start_s: f64,
    end_s: f64,
    speaker: Option<&str>,
    config: &PostProcessConfig,
) -> Vec<Segment> {
    if texts.is_empty() {
        return Vec::new();
    }

    let count = texts.len();
    let duration = (end_s - start_s).max(0.0);

    let mut weights: Vec<f64> = texts.iter().map(|t| effective_len(t)).collect();
    if weights.iter().sum::<f64>() <= 0.0 {
        // Punctuation-only chunk: fall back to an even split.
        weights = vec![1.0; count];
    }
    let total_weight: f64 = weights.iter().sum();

    // Reserve `min_duration` for every cue (spec §5.4 step 5) and split the rest
    // proportionally, so the result stays inside `[start_s, end_s]` and every cue
    // is as close to the requested minimum as the chunk budget allows.
    let reserved = (config.min_duration * count as f64).min(duration);
    let variable = duration - reserved;
    let floor = reserved / count as f64;

    let mut segments = Vec::with_capacity(count);
    let mut cursor = start_s;
    for (index, (text, weight)) in texts.iter().zip(weights.iter()).enumerate() {
        let end = if index + 1 == count {
            end_s
        } else {
            cursor + floor + variable * (weight / total_weight)
        };
        let end = end.max(cursor);
        segments.push(Segment {
            start: cursor,
            end,
            text: text.clone(),
            speaker: speaker.map(str::to_string),
        });
        cursor = end;
    }

    segments
}

/// Build cues from word timings: **no estimation**.
///
/// A cue starts at its first word and ends at its last, so the boundary is where
/// the speech actually is. Boundaries are chosen by, in priority order:
///
/// 1. **A pause longer than `gap_threshold`** between two words. The aligner
///    emits no punctuation (see [`crate::engine::words`]), so pauses are the
///    primary signal — this is the same rule the old Python engine applied to
///    Paraformer's per-character timestamps (`pause_threshold = 400 ms`).
/// 2. **A cue reaching `max_duration` or `max_chars`**, so one long unbroken
///    utterance still becomes readable cues.
///
/// `gap_threshold` is deliberately **not** used here: it is a merge threshold for
/// chunk windows, and the measured pauses between words are far shorter than the
/// pauses between chunks. [`PostProcessConfig::word_gap_threshold`] owns the
/// split decision and documents the measurements.
///
/// Words are assumed ordered by start time ([`crate::engine::words::parse_words`]
/// guarantees it). Concatenation is script-aware: CJK words are single
/// characters and must not be space-joined.
pub fn group_words_into_cues(
    words: &[Word],
    speaker: Option<&str>,
    config: &PostProcessConfig,
) -> Vec<Segment> {
    if words.is_empty() {
        return Vec::new();
    }

    let mut cues: Vec<Segment> = Vec::new();
    let mut current: Vec<&Word> = Vec::new();

    let flush = |current: &mut Vec<&Word>, cues: &mut Vec<Segment>| {
        if current.is_empty() {
            return;
        }
        let start = current.first().map(|word| word.start).unwrap_or(0.0);
        let end = current.last().map(|word| word.end).unwrap_or(start);
        let text = crate::engine::words::join_words(current.iter().map(|word| word.text.as_str()));
        let text = text.trim().to_string();
        current.clear();
        if text.is_empty() {
            return;
        }
        cues.push(Segment {
            start,
            end: end.max(start),
            text,
            speaker: speaker.map(str::to_string),
        });
    };

    for word in words {
        if let Some(previous) = current.last() {
            let gap = word.start - previous.end;
            let would_start = current.first().map(|first| first.start).unwrap_or(word.start);
            let duration = word.end - would_start;
            let chars = current
                .iter()
                .map(|word| char_count(&word.text))
                .sum::<usize>()
                + char_count(&word.text);

            // Never let a cue grow past the configured ceiling, and always
            // break on a real pause. A cue of exactly one word is exempt from
            // the length rules, or a single over-long word could never be
            // emitted at all.
            let pause_break = gap > config.word_gap_threshold;
            let length_break = !current.is_empty()
                && (duration > config.max_duration || chars > config.max_chars);
            if pause_break || length_break {
                flush(&mut current, &mut cues);
            }
        }
        current.push(word);
    }
    flush(&mut current, &mut cues);

    cues
}

/// Merge adjacent cues separated by less than `gap_threshold` when the result
/// stays within `max_chars` / `max_duration`.
pub fn merge_short(segments: Vec<Segment>, config: &PostProcessConfig) -> Vec<Segment> {
    if segments.len() <= 1 {
        return segments;
    }

    let mut merged: Vec<Segment> = Vec::with_capacity(segments.len());
    let mut i = 0;
    while i < segments.len() {
        let mut current = segments[i].clone();
        let mut current_duration = current.end - current.start;
        let mut current_chars = char_count(&current.text);

        while (current_duration < config.min_duration || current_chars < MIN_MERGE_CHARS)
            && i + 1 < segments.len()
        {
            let next = &segments[i + 1];
            let gap = next.start - current.end;
            let combined_duration = next.end - current.start;
            let combined_chars = current_chars + char_count(&next.text);

            // Speaker-aware merge (design §6.5 / §13 risk 8): only fuse cues
            // that belong to the same speaker. `None == None` is true, so the
            // no-speaker path behaves exactly as before and every existing
            // golden is unaffected; behaviour only changes when at least one
            // side carries a label.
            let same_speaker = current.speaker == next.speaker;

            if same_speaker
                && gap < config.gap_threshold
                && combined_duration <= config.max_duration
                && combined_chars <= config.max_chars
            {
                // Python concatenates without a separator; kept identical so the
                // port stays comparable against `_merge_short_segments`.
                current.end = next.end;
                current.text.push_str(&next.text);
                current_duration = current.end - current.start;
                current_chars = combined_chars;
                i += 1;
            } else {
                break;
            }
        }

        merged.push(current);
        i += 1;
    }

    merged
}

/// Final pass: enforce `min_duration` and remove overlaps.
pub fn enforce_constraints(mut segments: Vec<Segment>, config: &PostProcessConfig) -> Vec<Segment> {
    for i in 0..segments.len() {
        let min_end = segments[i].start + config.min_duration;
        if segments[i].end < min_end {
            // Never grow a cue past the next cue's start, otherwise the
            // non-overlap guarantee would break.
            let limit = segments.get(i + 1).map(|next| next.start);
            segments[i].end = match limit {
                Some(next_start) => min_end.min(next_start).max(segments[i].start),
                None => min_end,
            };
        }
    }

    for i in 1..segments.len() {
        if segments[i].start < segments[i - 1].end {
            segments[i].start = segments[i - 1].end;
        }
        if segments[i].end < segments[i].start {
            segments[i].end = segments[i].start;
        }
    }

    segments
}

fn char_count(text: &str) -> usize {
    text.chars().count()
}

fn is_cjk(ch: char) -> bool {
    matches!(
        ch as u32,
        0x4E00..=0x9FFF | 0x3400..=0x4DBF | 0x20000..=0x2A6DF | 0xF900..=0xFAFF
    )
}

/// Split after every terminal punctuation mark and keep it attached to the
/// preceding sentence. Unlike Python's `_split_text_by_punctuation`, a `.` only
/// ends a sentence when followed by whitespace or the end of the text (spec
/// §5.4), so decimals such as `3.14` stay intact.
fn split_on_terminals(text: &str) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    let mut out = Vec::new();
    let mut current = String::new();

    for (index, ch) in chars.iter().enumerate() {
        current.push(*ch);
        let terminal = matches!(ch, '。' | '！' | '？' | '!' | '?' | '；' | ';')
            || (*ch == '.' && chars.get(index + 1).map_or(true, |next| next.is_whitespace()));
        if terminal {
            push_trimmed(&mut out, &mut current);
        }
    }
    push_trimmed(&mut out, &mut current);

    out
}

/// Split after `，,、` (spec §5.4), keeping the punctuation attached.
fn split_on_minor(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();

    for ch in text.chars() {
        current.push(ch);
        if matches!(ch, '，' | ',' | '、') {
            push_trimmed(&mut out, &mut current);
        }
    }
    push_trimmed(&mut out, &mut current);

    out
}

fn push_trimmed(out: &mut Vec<String>, current: &mut String) {
    let trimmed = current.trim();
    if !trimmed.is_empty() {
        out.push(trimmed.to_string());
    }
    current.clear();
}

enum Unit {
    /// A whitespace-delimited latin word (punctuation may be attached).
    Word(String),
    /// A single CJK ideograph.
    Cjk(char),
}

/// Hard-split a still-over-long cue: CJK by character, latin by word (spec §5.4).
fn hard_split(text: &str, max_chars: usize) -> Vec<String> {
    let max_chars = max_chars.max(1);
    let mut chunks: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut current_len = 0usize;
    let mut previous_word = false;

    for unit in tokenize_units(text) {
        let (piece, is_word) = match unit {
            Unit::Word(word) => (word, true),
            Unit::Cjk(ch) => (ch.to_string(), false),
        };
        let piece_len = char_count(&piece);
        let separator = usize::from(previous_word && is_word && !current.is_empty());

        if current_len + separator + piece_len <= max_chars {
            if separator == 1 {
                current.push(' ');
                current_len += 1;
            }
            current.push_str(&piece);
            current_len += piece_len;
            previous_word = is_word;
        } else if piece_len > max_chars {
            // A single unit larger than the budget: fall back to a raw char split.
            if !current.is_empty() {
                chunks.push(std::mem::take(&mut current));
                current_len = 0;
            }
            let mut buffer = String::new();
            let mut length = 0usize;
            for ch in piece.chars() {
                if length == max_chars {
                    chunks.push(std::mem::take(&mut buffer));
                    length = 0;
                }
                buffer.push(ch);
                length += 1;
            }
            if !buffer.is_empty() {
                chunks.push(buffer);
            }
            previous_word = false;
        } else {
            if !current.is_empty() {
                chunks.push(std::mem::take(&mut current));
            }
            current = piece;
            current_len = piece_len;
            previous_word = is_word;
        }
    }

    if !current.is_empty() {
        chunks.push(current);
    }

    chunks
}

fn tokenize_units(text: &str) -> Vec<Unit> {
    let mut units = Vec::new();
    let mut buffer = String::new();
    for ch in text.chars() {
        if is_cjk(ch) {
            if !buffer.is_empty() {
                units.push(Unit::Word(std::mem::take(&mut buffer)));
            }
            units.push(Unit::Cjk(ch));
        } else if ch.is_whitespace() {
            if !buffer.is_empty() {
                units.push(Unit::Word(std::mem::take(&mut buffer)));
            }
        } else {
            buffer.push(ch);
        }
    }
    if !buffer.is_empty() {
        units.push(Unit::Word(buffer));
    }
    units
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::RawChunk;
    use crate::subtitle::postprocess as compose;
    /// The estimated-timing pipeline is the default these tests exercise; word
    /// grouping has its own tests below.
    fn run_pipeline(chunks: &[crate::engine::RawChunk], config: &PostProcessConfig) -> Vec<Segment> {
        crate::subtitle::postprocess(chunks, &[], config)
    }


    fn config() -> PostProcessConfig {
        PostProcessConfig::default()
    }

    fn chunk(start_s: f64, end_s: f64, text: &str) -> RawChunk {
        RawChunk {
            start_s,
            end_s,
            text: text.to_string(),
            speaker: None,
            language: None,
        }
    }

    fn assert_monotonic(segments: &[Segment]) {
        for (index, segment) in segments.iter().enumerate() {
            assert!(
                segment.end > segment.start,
                "cue {index} is not positive-length: {segment:?}"
            );
            if index + 1 < segments.len() {
                assert!(
                    segment.end <= segments[index + 1].start,
                    "cue {index} overlaps cue {}: {:?}",
                    index + 1,
                    segments
                );
                assert!(
                    segment.start < segments[index + 1].start,
                    "cue starts are not strictly increasing at {index}: {:?}",
                    segments
                );
            }
        }
    }

    #[test]
    fn clean_text_strips_tags_and_drops_blank() {
        assert_eq!(
            clean_text("<|zh|><|NEUTRAL|><|Speech|><|withitn|>开放时间"),
            Some("开放时间".to_string())
        );
        assert_eq!(clean_text("  hello  "), Some("hello".to_string()));
        assert_eq!(clean_text("   "), None);
        assert_eq!(clean_text("<|zh|>"), None);
        // A lone `<|` without a closing `|>` is preserved verbatim.
        assert_eq!(clean_text("a<|b"), Some("a<|b".to_string()));
    }

    /// The language tag is read BEFORE `clean_text` runs; the two must not fight
    /// over the same `<|…|>` syntax (design §5).
    #[test]
    fn language_tag_is_extracted_before_cleaning() {
        let raw = "<|zh|><|NEUTRAL|><|Speech|><|withitn|>开放时间";

        assert_eq!(extract_language_tag(raw).as_deref(), Some("zh"));
        // ...and cleaning still removes every tag, unchanged.
        assert_eq!(clean_text(raw).as_deref(), Some("开放时间"));
        // The tag has to come first, or cleaning loses it.
        assert_eq!(extract_language_tag(&clean_text(raw).unwrap()), None);
    }

    /// Only real language tokens count: the same delimiters carry emotion,
    /// event and ITN markers, and a wrong guess here would skip a translation
    /// that is actually needed.
    #[test]
    fn only_language_tokens_are_extracted() {
        assert_eq!(extract_language_tag("<|en|>hello"), Some("en".to_string()));
        assert_eq!(extract_language_tag("<|YUE|>hi"), Some("yue".to_string()));
        assert_eq!(extract_language_tag("<|NEUTRAL|>hi"), None);
        assert_eq!(extract_language_tag("<|Speech|>hi"), None);
        assert_eq!(extract_language_tag("<|withitn|>hi"), None);
        assert_eq!(extract_language_tag("<|auto|>hi"), None);
        assert_eq!(extract_language_tag("plain text"), None);
        assert_eq!(extract_language_tag(""), None);
        // A malformed tail must not panic or invent a tag.
        assert_eq!(extract_language_tag("<|"), None);
        assert_eq!(extract_language_tag("<|zh"), None);
        // The second tag is found when the first is not a language.
        assert_eq!(
            extract_language_tag("<|NEUTRAL|><|ko|>안녕").as_deref(),
            Some("ko")
        );
    }

    /// Majority wins across chunks; a tie keeps the first tag that was seen.
    #[test]
    fn dominant_language_uses_majority_then_first_seen() {
        let tags = |values: &[Option<&'static str>]| dominant_language(values.iter().copied());

        assert_eq!(tags(&[Some("zh"), Some("zh"), Some("en")]).as_deref(), Some("zh"));
        assert_eq!(tags(&[Some("en"), Some("en"), Some("zh")]).as_deref(), Some("en"));
        // Tie: the first seen wins, so the answer is stable.
        assert_eq!(tags(&[Some("zh"), Some("en")]).as_deref(), Some("zh"));
        assert_eq!(tags(&[Some("en"), Some("zh")]).as_deref(), Some("en"));
        // Untagged chunks are simply skipped, not counted as a language.
        assert_eq!(
            tags(&[None, Some("ja"), None, Some("ja"), None]).as_deref(),
            Some("ja")
        );
        assert_eq!(dominant_language([Some(" "), Some("")]), None);
    }

    /// No tag at all (the moss / direct path) means "unknown", which is the
    /// documented signal to translate anyway — it must never be an error.
    #[test]
    fn dominant_language_is_none_without_any_tag() {
        assert_eq!(dominant_language(std::iter::empty()), None);
        assert_eq!(dominant_language([None, None, None]), None);
    }

    #[test]
    fn effective_len_weights_cjk_latin_digits_and_punctuation() {
        assert_eq!(effective_len("你好"), 2.0);
        assert_eq!(effective_len("hello world"), 2.0);
        assert_eq!(effective_len("3.14"), 1.5);
        assert_eq!(effective_len("你好 world"), 3.0);
        assert_eq!(effective_len("café"), 1.0);
        assert_eq!(effective_len("，。！？"), 0.0);
        assert_eq!(effective_len(""), 0.0);
    }

    #[test]
    fn split_keeps_decimal_point_together() {
        let sentences = split_sentences("圆周率是 3.14 呀。真的吗？", 50);
        assert_eq!(sentences, vec!["圆周率是 3.14 呀。", "真的吗？"]);
    }

    #[test]
    fn split_is_noop_for_short_text() {
        assert_eq!(split_sentences("你好，世界。", 50), vec!["你好，世界。"]);
    }

    #[test]
    fn over_long_sentence_is_hard_split() {
        let long_zh = "一".repeat(120);
        let sentences = split_sentences(&long_zh, 50);
        assert!(sentences.len() >= 3);
        assert!(sentences.iter().all(|s| s.chars().count() <= 50));
        assert_eq!(sentences.concat(), long_zh);

        let long_en = "alpha beta gamma delta epsilon zeta eta theta iota kappa lambda mu nu xi omicron pi rho sigma tau upsilon phi chi psi omega";
        let sentences = split_sentences(long_en, 20);
        assert!(sentences.len() > 1);
        assert!(sentences.iter().all(|s| s.chars().count() <= 20));
        // Hard splits keep whole words (never cut mid-word) and rejoin losslessly.
        assert_eq!(sentences.join(" "), long_en);
        assert!(sentences.iter().all(|s| !s.contains("  ")));
    }

    #[test]
    fn postprocess_chinese_chunk() {
        let chunks = vec![chunk(0.0, 6.0, "开放时间早上9点至下午5点。今天天气不错。")];
        let segments = run_pipeline(&chunks, &config());
        assert!(!segments.is_empty());
        assert_eq!(segments[0].start, 0.0);
        assert_eq!(segments.last().unwrap().end, 6.0);
        assert_monotonic(&segments);
    }

    #[test]
    fn postprocess_english_chunk() {
        let chunks = vec![chunk(1.0, 9.0, "Hello there. How are you doing today?")];
        let segments = run_pipeline(&chunks, &config());
        assert!(segments.len() >= 2);
        assert_eq!(segments[0].start, 1.0);
        assert_eq!(segments.last().unwrap().end, 9.0);
        assert_monotonic(&segments);
    }

    #[test]
    fn postprocess_mixed_cjk_latin_chunk() {
        let chunks = vec![chunk(0.0, 10.0, "我们用 Rust 重写了 engine。Now it is fast.")];
        let segments = run_pipeline(&chunks, &config());
        assert!(!segments.is_empty());
        assert_eq!(segments.last().unwrap().end, 10.0);
        assert_monotonic(&segments);
    }

    #[test]
    fn postprocess_over_long_sentence_stays_bounded() {
        let long = format!("{}。", "这是一句非常长的中文测试文本".repeat(10));
        let chunks = vec![chunk(0.0, 12.0, &long)];
        let segments = run_pipeline(&chunks, &config());
        assert!(segments.len() > 1);
        assert!(segments.iter().all(|s| s.text.chars().count() <= 50));
        assert!(segments
            .iter()
            .all(|s| s.start >= 0.0 && s.end <= 12.0));
        assert_monotonic(&segments);
    }

    #[test]
    fn postprocess_empty_and_whitespace_text() {
        let chunks = vec![
            chunk(0.0, 1.0, "   "),
            chunk(1.0, 2.0, "<|zh|>"),
            chunk(2.0, 3.0, ""),
        ];
        assert!(run_pipeline(&chunks, &config()).is_empty());
        assert!(run_pipeline(&[], &config()).is_empty());
    }

    #[test]
    fn postprocess_is_strictly_monotonic_across_chunks() {
        let chunks = vec![
            chunk(0.0, 5.0, "第一段字幕。"),
            chunk(5.0, 11.0, "第二段字幕，写得更长一些，用来测试时间分配是否会越界。"),
            chunk(11.0, 14.5, "Third chunk ends here."),
        ];
        let segments = run_pipeline(&chunks, &config());
        assert!(!segments.is_empty());
        assert_monotonic(&segments);
        assert!(segments.iter().all(|s| s.start >= 0.0 && s.end <= 14.5));
    }

    #[test]
    fn merge_short_joins_close_tiny_cues() {
        let segments = vec![
            Segment {
                start: 0.0,
                end: 0.1,
                text: "嗯".to_string(),
                speaker: None,
            },
            Segment {
                start: 0.2,
                end: 2.0,
                text: "我们继续。".to_string(),
                speaker: None,
            },
        ];
        let merged = merge_short(segments, &config());
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].text, "嗯我们继续。");
        assert_eq!(merged[0].end, 2.0);
    }

    /// Same speaker: the label-aware precondition must still allow a merge
    /// (otherwise turning on diarization would fragment every cue).
    #[test]
    fn merge_short_still_joins_same_speaker_cues() {
        let segments = vec![
            Segment {
                start: 0.0,
                end: 0.1,
                text: "嗯".to_string(),
                speaker: Some("Speaker A".to_string()),
            },
            Segment {
                start: 0.2,
                end: 2.0,
                text: "我们继续。".to_string(),
                speaker: Some("Speaker A".to_string()),
            },
        ];
        let merged = merge_short(segments, &config());
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].text, "嗯我们继续。");
        assert_eq!(merged[0].speaker.as_deref(), Some("Speaker A"));
    }

    /// Different speakers: the adjacent short cues must NOT be fused, or the
    /// second speaker's words would be attributed to the first (design §6.5).
    #[test]
    fn merge_short_never_joins_across_speakers() {
        let segments = vec![
            Segment {
                start: 0.0,
                end: 0.1,
                text: "嗯".to_string(),
                speaker: Some("Speaker A".to_string()),
            },
            Segment {
                start: 0.2,
                end: 2.0,
                text: "该你了。".to_string(),
                speaker: Some("Speaker B".to_string()),
            },
        ];
        let merged = merge_short(segments, &config());
        assert_eq!(merged.len(), 2, "cues from different speakers must stay apart");
        assert_eq!(merged[0].speaker.as_deref(), Some("Speaker A"));
        assert_eq!(merged[1].speaker.as_deref(), Some("Speaker B"));
    }

    /// A labelled cue must not absorb an unlabelled one either: the two are not
    /// known to share a speaker.
    #[test]
    fn merge_short_does_not_join_labelled_with_unlabelled() {
        let segments = vec![
            Segment {
                start: 0.0,
                end: 0.1,
                text: "嗯".to_string(),
                speaker: Some("Speaker A".to_string()),
            },
            Segment {
                start: 0.2,
                end: 2.0,
                text: "没有标签。".to_string(),
                speaker: None,
            },
        ];
        let merged = merge_short(segments, &config());
        assert_eq!(merged.len(), 2);
    }

    /// The composer must propagate a chunk's label into every cue it produces.
    #[test]
    fn postprocess_propagates_chunk_speaker_to_segments() {
        let chunks = vec![
            RawChunk {
                start_s: 0.0,
                end_s: 2.0,
                text: "第一句。第二句。".to_string(),
                speaker: Some("Speaker B".to_string()),
                language: None,
            },
            RawChunk {
                start_s: 2.0,
                end_s: 4.0,
                text: "第三句。".to_string(),
                speaker: None,
                language: None,
            },
        ];
        let segments = compose(&chunks, &[], &config());

        assert!(!segments.is_empty());
        assert!(
            segments
                .iter()
                .any(|segment| segment.speaker.as_deref() == Some("Speaker B")),
            "the labelled chunk's cues must carry its speaker: {segments:?}"
        );
    }

    #[test]
    fn enforce_constraints_lifts_short_cues_and_removes_overlap() {
        // The gap after the first cue gives room to reach `min_duration`.
        let segments = vec![
            Segment {
                start: 0.0,
                end: 0.1,
                text: "a".to_string(),
                speaker: None,
            },
            Segment {
                start: 0.5,
                end: 1.0,
                text: "b".to_string(),
                speaker: None,
            },
        ];
        let fixed = enforce_constraints(segments, &config());
        assert_eq!(fixed[0].end, 0.3);
        assert!(fixed[0].end <= fixed[1].start);
        assert!(fixed[1].start < fixed[1].end);
    }

    #[test]
    fn enforce_constraints_never_creates_overlap() {
        // Overlapping input: non-overlap wins over `min_duration`.
        let segments = vec![
            Segment {
                start: 0.0,
                end: 0.1,
                text: "a".to_string(),
                speaker: None,
            },
            Segment {
                start: 0.05,
                end: 1.0,
                text: "b".to_string(),
                speaker: None,
            },
        ];
        let fixed = enforce_constraints(segments, &config());
        assert!(fixed[0].end <= fixed[1].start);
        assert!(fixed[1].start < fixed[1].end);
        assert_monotonic(&fixed);
    }

    // --- word-level grouping ------------------------------------------------

    fn word(start: f64, end: f64, text: &str) -> crate::engine::words::Word {
        crate::engine::words::Word {
            index: 0,
            start,
            end,
            text: text.to_string(),
        }
    }

    /// The core claim of word-level timing: a cue's edges are the edges of its
    /// words. Nothing is estimated, so a cue cannot drift into the silence around
    /// it the way proportional distribution does.
    #[test]
    fn cue_edges_come_from_the_words_not_from_the_window() {
        let words = vec![word(1.0, 1.2, "你"), word(1.2, 1.6, "好")];
        let cues = group_words_into_cues(&words, None, &config());

        assert_eq!(cues.len(), 1);
        assert_eq!(cues[0].start, 1.0);
        assert_eq!(cues[0].end, 1.6);
        assert_eq!(cues[0].text, "你好");
    }

    /// Pauses at real sentence boundaries split the cue, because the aligner
    /// emits no punctuation and a silence is the only sentence signal it gives.
    ///
    /// The values are the ones measured on real speech (see
    /// `word-timestamps-poc.md`): 0.40 s at a full stop, 0.16 s at a comma-like
    /// break. Both are above `word_gap_threshold`.
    #[test]
    fn sentence_boundary_pauses_split_a_cue() {
        let words = vec![
            word(0.0, 0.4, "第"),
            word(0.4, 0.8, "一"),
            // The measured full-stop pause.
            word(1.2, 1.6, "第"),
            word(1.6, 1.85, "二"),
            // The measured comma-like pause (0.16 s).
            word(2.01, 2.4, "三"),
        ];
        let cues = group_words_into_cues(&words, None, &config());

        assert_eq!(cues.len(), 3, "both measured boundaries must split: {cues:?}");
        assert_eq!(cues[0].text, "第一");
        assert_eq!(cues[0].start, 0.0);
        assert_eq!(cues[0].end, 0.8);
        assert_eq!(cues[1].start, 1.2, "the second cue starts after the pause");
        assert_eq!(cues[2].start, 2.01);
    }

    /// Coarticulation inside a phrase must NOT split, or every character becomes
    /// its own cue. Measured in-phrase pauses were 0.08 s or 0.
    #[test]
    fn in_phrase_gaps_do_not_split() {
        let words = vec![
            word(0.0, 0.2, "我"),
            word(0.2, 0.4, "们"),
            // 0.08 s: measured in-phrase pause.
            word(0.48, 0.7, "来"),
        ];
        let cues = group_words_into_cues(&words, None, &config());

        assert_eq!(cues.len(), 1, "0.08 s is not a sentence break: {cues:?}");
        assert_eq!(cues[0].text, "我们来");
    }

    /// The split threshold is separate from the merge threshold, and the two are
    /// deliberately far apart: a word-level pause is an order of magnitude
    /// shorter than a chunk-level gap.
    #[test]
    fn word_split_threshold_is_independent_of_the_merge_threshold() {
        let config = PostProcessConfig::default();
        assert!(
            config.word_gap_threshold < config.gap_threshold,
            "measured word pauses (<=0.48 s) are shorter than chunk gaps"
        );
        assert!(
            config.word_gap_threshold > 0.1,
            "in-phrase pauses reach 0.08 s, so the threshold must clear them"
        );
    }

    /// `max_chars` caps a cue even with no pause at all, so an unbroken
    /// utterance still becomes readable.
    #[test]
    fn max_chars_caps_a_cue_without_any_pause() {
        let config = PostProcessConfig {
            max_chars: 3,
            ..PostProcessConfig::default()
        };
        // Six one-character CJK words, all contiguous.
        let words: Vec<_> = (0..6)
            .map(|i| word(i as f64 * 0.1, (i + 1) as f64 * 0.1, "字"))
            .collect();

        let cues = group_words_into_cues(&words, None, &config);

        assert_eq!(cues.len(), 2, "3 chars per cue: {cues:?}");
        assert_eq!(cues[0].text, "字字字");
        assert_eq!(cues[1].text, "字字字");
        assert!(cues[0].end <= cues[1].start);
    }

    /// A single word longer than `max_chars` must still be emitted: the length
    /// rule cannot be applied to the first word or progress is impossible.
    #[test]
    fn an_oversized_single_word_is_still_emitted() {
        let config = PostProcessConfig {
            max_chars: 1,
            ..PostProcessConfig::default()
        };
        let words = vec![word(0.0, 0.5, "supercalifragilistic")];
        let cues = group_words_into_cues(&words, None, &config);

        assert_eq!(cues.len(), 1);
        assert_eq!(cues[0].text, "supercalifragilistic");
    }

    /// `max_duration` caps a cue whose words never pause, e.g. fast speech.
    #[test]
    fn max_duration_caps_a_cue_without_any_pause() {
        let config = PostProcessConfig {
            max_duration: 1.0,
            max_chars: 1000,
            ..PostProcessConfig::default()
        };
        let words: Vec<_> = (0..10)
            .map(|i| word(i as f64 * 0.3, (i + 1) as f64 * 0.3, "w"))
            .collect();

        let cues = group_words_into_cues(&words, None, &config);

        assert!(cues.len() > 1, "3 s of speech must split: {cues:?}");
        for cue in &cues {
            assert!(
                cue.end - cue.start <= 1.3,
                "cue over the ceiling: {cue:?}"
            );
        }
    }

    /// Speaker labels survive the grouping, so `[Speaker A]: ` still renders.
    #[test]
    fn speaker_is_carried_onto_every_cue() {
        let words = vec![word(0.0, 0.3, "hi"), word(1.5, 1.8, "there")];
        let cues = group_words_into_cues(&words, Some("Speaker A"), &config());

        assert_eq!(cues.len(), 2);
        assert!(cues
            .iter()
            .all(|cue| cue.speaker.as_deref() == Some("Speaker A")));
    }

    /// Latin words are space-joined, CJK is not — the same cue can hold both.
    #[test]
    fn cue_text_joins_per_script() {
        let words = vec![
            word(0.0, 0.2, "我"),
            word(0.2, 0.4, "用"),
            word(0.4, 0.7, "Mac"),
        ];
        let cues = group_words_into_cues(&words, None, &config());

        assert_eq!(cues[0].text, "我用Mac");
    }

    #[test]
    fn no_words_yields_no_cues() {
        assert!(group_words_into_cues(&[], None, &config()).is_empty());
    }
}
