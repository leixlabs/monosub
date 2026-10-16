//! Sentence splitting, proportional timestamp estimation and merging.
//!
//! Task: P1-10. Ported from `core-engine/monosub_core/transcriber.py`
//! (`_split_text_by_punctuation`, `_estimate_duration`, `_merge_short_segments`)
//! per docs/tech-spec/refactor-audiocpp.md §5.4.

use super::{PostProcessConfig, Segment};

/// Python `_merge_short_segments` also merges any cue shorter than 5 characters
/// (hard-coded next to `MERGE_CONFIG`), in addition to `min_duration`.
const MIN_MERGE_CHARS: usize = 5;

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
    use crate::subtitle::{postprocess as run_pipeline, PostProcessConfig};

    fn config() -> PostProcessConfig {
        PostProcessConfig::default()
    }

    fn chunk(start_s: f64, end_s: f64, text: &str) -> RawChunk {
        RawChunk {
            start_s,
            end_s,
            text: text.to_string(),
            speaker: None,
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
            },
            RawChunk {
                start_s: 2.0,
                end_s: 4.0,
                text: "第三句。".to_string(),
                speaker: None,
            },
        ];
        let segments = compose(&chunks, &config());

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
}
