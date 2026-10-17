//! Parsing a translation response back into numbered cues.
//!
//! Task: T3-3. See docs/tech-spec/translation-design.md §4 ("result validation")
//! and the PoC notes on output-format stability.
//!
//! Two shapes are accepted, in this order:
//!
//! 1. **A JSON payload** (array of strings, array of objects, or an
//!    index → text object). No measured response used it — `response_format`
//!    and GBNF grammar both failed against HY-MT — but another model family may
//!    answer that way, and refusing a perfectly readable answer would be worse
//!    than the extra branch.
//! 2. **Numbered lines**, which is what the model actually emits: one
//!    `"<n>. <text>"` line per cue.
//!
//! A non-blank line with no leading number continues the previous cue (the
//! model sometimes wraps a translation). A blank line ends the current cue, so
//! anything after it that has no number — a preamble, or a trailing "hope this
//! helps" — is dropped rather than smuggled into a translation.
//!
//! Everything is pure; nothing here knows about HTTP or the model.

use std::collections::BTreeMap;

/// Parse `raw` into `index -> translated text`.
///
/// `first_index` anchors a positional answer (a bare JSON array of strings, or
/// elements with no explicit number) onto the batch that was sent. Missing or
/// extra indices are simply absent/extra in the result — the caller owns
/// validation, because only it knows how many cues the batch had.
pub fn parse_translations(raw: &str, first_index: usize) -> BTreeMap<usize, String> {
    if let Some(from_json) = parse_json(raw, first_index) {
        if !from_json.is_empty() {
            return from_json;
        }
    }

    parse_numbered_lines(raw)
}

/// Parse the two square-bracket/brace JSON forms, or `None` when `raw` is not
/// JSON at all.
fn parse_json(raw: &str, first_index: usize) -> Option<BTreeMap<usize, String>> {
    let trimmed = raw.trim();
    if !trimmed.starts_with(['[', '{']) {
        return None;
    }

    let value: serde_json::Value = serde_json::from_str(trimmed).ok()?;
    let mut out = BTreeMap::new();

    match value {
        serde_json::Value::Array(items) => {
            for (offset, item) in items.iter().enumerate() {
                let positional = first_index + offset;
                match item {
                    serde_json::Value::String(text) => {
                        out.insert(positional, text.trim().to_string());
                    }
                    serde_json::Value::Object(map) => {
                        let index = json_index(map).unwrap_or(positional);
                        if let Some(text) = json_text(map) {
                            out.insert(index, text);
                        }
                    }
                    _ => {}
                }
            }
        }
        serde_json::Value::Object(map) => {
            for (key, item) in &map {
                let Ok(index) = key.trim().parse::<usize>() else {
                    continue;
                };
                let text = match item {
                    serde_json::Value::String(text) => text.trim().to_string(),
                    serde_json::Value::Object(inner) => match json_text(inner) {
                        Some(text) => text,
                        None => continue,
                    },
                    _ => continue,
                };
                out.insert(index, text);
            }
        }
        _ => return None,
    }

    Some(out)
}

/// The explicit cue number in a JSON object, under any of the spellings a model
/// is likely to use.
fn json_index(map: &serde_json::Map<String, serde_json::Value>) -> Option<usize> {
    ["index", "id", "number", "n", "line"]
        .iter()
        .find_map(|key| map.get(*key).and_then(|value| value.as_u64()))
        .map(|value| value as usize)
}

/// The translated text in a JSON object.
fn json_text(map: &serde_json::Map<String, serde_json::Value>) -> Option<String> {
    ["text", "translation", "target", "content", "translated"]
        .iter()
        .find_map(|key| map.get(*key).and_then(|value| value.as_str()))
        .map(|text| text.trim().to_string())
}

/// Parse `"<n><separator> <text>"` lines, attaching continuation lines to the
/// cue they belong to.
///
/// Accepts the separators the model was observed to use (`1.` `1)` `1、` `1：`)
/// and any leading whitespace — the equivalent of the PoC's
/// `^\s*(\d+)\s*[.)、:：]\s*(.*)$`, written by hand because the crate has no
/// regex dependency.
fn parse_numbered_lines(raw: &str) -> BTreeMap<usize, String> {
    let mut out: BTreeMap<usize, String> = BTreeMap::new();
    // The cue a continuation line would extend; `None` before the first cue and
    // after a blank line, which is what makes a trailing note unambiguous.
    let mut current: Option<usize> = None;

    for line in raw.lines() {
        if line.trim().is_empty() {
            current = None;
            continue;
        }

        if let Some((index, text)) = split_numbered_line(line) {
            out.insert(index, text);
            current = Some(index);
            continue;
        }

        match current {
            Some(index) => {
                let text = out.entry(index).or_default();
                if !text.is_empty() {
                    text.push(' ');
                }
                text.push_str(line.trim());
            }
            // A preamble line, or a trailing explanation after a blank line.
            None => continue,
        }
    }

    out
}

/// A lone cue is answered without numbering (see `prompt::single_cue_prompt`), so
/// the whole answer is the translation. Only the obvious non-answers are
/// rejected: an empty body, or an echo of the instruction.
///
/// Returns `None` when the answer cannot be trusted, which makes the caller keep
/// the original text rather than ship the instruction as a subtitle.
pub fn parse_single_translation(raw: &str, source_text: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }

    // A model that answers in the numbered shape anyway is handled by the normal
    // path; a `1. ` prefix would otherwise end up inside the subtitle.
    let body = match split_numbered_line(trimmed.lines().next().unwrap_or_default()) {
        Some((1, text)) => {
            let mut rest = text;
            for line in trimmed.lines().skip(1) {
                if line.trim().is_empty() {
                    continue;
                }
                rest.push(' ');
                rest.push_str(line.trim());
            }
            rest.trim().to_string()
        }
        _ => trimmed
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .collect::<Vec<_>>()
            .join(" "),
    };

    if body.is_empty() {
        return None;
    }
    // An answer that merely restates the source is not a translation: measured,
    // this is exactly how the model fails on a single batched cue, and accepting
    // it would look like success while shipping untranslated text.
    if body == source_text.trim() {
        return None;
    }
    // The prompt instructions coming back verbatim is the other measured
    // failure mode (zh→en returned the instruction line itself).
    if is_instruction_echo(&body) {
        return None;
    }

    Some(body)
}

/// True when an answer is really the prompt's own instruction text.
fn is_instruction_echo(text: &str) -> bool {
    const MARKERS: [&str; 6] = [
        "输出必须",
        "不要额外解释",
        "output must be exactly",
        "do not add explanations",
        "without additional explanation",
        "注意只需要输出",
    ];
    let lowered = text.to_lowercase();
    MARKERS.iter().any(|marker| {
        lowered.contains(&marker.to_lowercase()) && text.chars().count() < 120
    })
}

/// `(index, first-line text)` for a numbered line, `None` otherwise.
fn split_numbered_line(line: &str) -> Option<(usize, String)> {
    let trimmed = line.trim_start();
    let digits_end = trimmed.find(|c: char| !c.is_ascii_digit())?;
    if digits_end == 0 {
        return None;
    }
    let index: usize = trimmed[..digits_end].parse().ok()?;

    let rest = trimmed[digits_end..].trim_start();
    let mut chars = rest.chars();
    if !matches!(chars.next(), Some('.' | ')' | '、' | ':' | '：')) {
        return None;
    }

    Some((index, chars.as_str().trim().to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The measured happy path: one line per cue, in order.
    #[test]
    fn parses_numbered_lines_in_order() {
        let raw = "1. Hello there.\n2. How are you?\n3. Goodbye.";
        let parsed = parse_translations(raw, 1);

        assert_eq!(parsed.len(), 3);
        assert_eq!(parsed[&1], "Hello there.");
        assert_eq!(parsed[&2], "How are you?");
        assert_eq!(parsed[&3], "Goodbye.");
    }

    /// The separators the model was observed to use are interchangeable.
    #[test]
    fn accepts_every_observed_separator() {
        let raw = "1. a\n2) b\n3、c\n4：d\n  5 : e";
        let parsed = parse_translations(raw, 1);

        assert_eq!(parsed.len(), 5);
        assert_eq!(parsed[&2], "b");
        assert_eq!(parsed[&3], "c");
        assert_eq!(parsed[&4], "d");
        assert_eq!(parsed[&5], "e");
    }

    /// A wrapped translation continues the previous cue instead of being lost.
    #[test]
    fn continuation_lines_extend_the_previous_cue() {
        let raw = "1. First part\ncontinued here\n2. Second";
        let parsed = parse_translations(raw, 1);

        assert_eq!(parsed[&1], "First part continued here");
        assert_eq!(parsed[&2], "Second");
    }

    /// A blank line ends a cue, so a trailing note cannot be folded into the
    /// last translation.
    #[test]
    fn blank_line_ends_the_cue_so_a_trailing_note_is_dropped() {
        let raw = "1. Hello\n2. World\n\nHope this helps!";
        let parsed = parse_translations(raw, 1);

        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[&2], "World");
    }

    /// A preamble before the first numbered line is dropped too.
    #[test]
    fn preamble_explanation_is_dropped() {
        let raw = "Here are the translations:\n\n1. Hello\n2. World";
        let parsed = parse_translations(raw, 1);

        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[&1], "Hello");
    }

    /// A missing cue is simply absent; the caller's count check is what turns
    /// that into a retry.
    #[test]
    fn missing_cue_is_absent_not_shifted() {
        let raw = "1. One\n3. Three";
        let parsed = parse_translations(raw, 1);

        assert_eq!(parsed.len(), 2);
        assert!(!parsed.contains_key(&2));
        assert_eq!(parsed[&3], "Three");
    }

    /// Out-of-order cues keep their own numbers: nothing is reassigned by
    /// position, so a reordered answer cannot silently scramble the subtitle.
    #[test]
    fn out_of_order_cues_keep_their_numbers() {
        let raw = "3. Three\n1. One\n2. Two";
        let parsed = parse_translations(raw, 1);

        let ordered: Vec<(usize, &str)> = parsed
            .iter()
            .map(|(index, text)| (*index, text.as_str()))
            .collect();
        assert_eq!(
            ordered,
            vec![(1, "One"), (2, "Two"), (3, "Three")],
            "the map is index-ordered, and the numbers decide the mapping"
        );
    }

    /// An extra cue is kept — the caller rejects the batch for a count mismatch,
    /// and inventing/merging a cue here would hide the drift.
    #[test]
    fn extra_cue_is_kept_for_the_caller_to_reject() {
        let raw = "1. One\n2. Two\n3. Three";
        let parsed = parse_translations(raw, 1);

        assert_eq!(parsed.len(), 3);
        assert_eq!(parsed[&3], "Three");
    }

    #[test]
    fn empty_output_yields_an_empty_map() {
        for raw in ["", "   ", "\n\n", "no numbers here at all"] {
            assert!(
                parse_translations(raw, 1).is_empty(),
                "raw {raw:?} must not invent a cue"
            );
        }
    }

    /// `[]` (what a grammar-constrained run returned) is empty, not an error.
    #[test]
    fn empty_json_array_yields_an_empty_map() {
        assert!(parse_translations("[]", 1).is_empty());
        assert!(parse_translations("{}", 1).is_empty());
    }

    /// Right-to-left text is embedded verbatim; nothing reverses or strips it.
    #[test]
    fn right_to_left_text_is_preserved() {
        let raw = "1. مرحبا بالعالم\n2. שלום עולם";
        let parsed = parse_translations(raw, 1);

        assert_eq!(parsed[&1], "مرحبا بالعالم");
        assert_eq!(parsed[&2], "שלום עולם");
    }

    /// The JSON fallbacks: bare strings, objects with an index, and a map.
    #[test]
    fn parses_json_answers() {
        let strings = parse_translations(r#"["one", "two", "three"]"#, 5);
        assert_eq!(strings[&5], "one");
        assert_eq!(strings[&7], "three");

        let objects = parse_translations(
            r#"[{"index": 3, "text": "three"}, {"text": "four"}]"#,
            1,
        );
        assert_eq!(objects[&3], "three");
        // No explicit index: anchored onto the batch start plus its offset.
        assert_eq!(objects[&2], "four");
        assert!(!objects.contains_key(&1), "the offset is not collapsed");

        let map = parse_translations(r#"{"1": "one", "2": {"text": "two"}}"#, 1);
        assert_eq!(map[&1], "one");
        assert_eq!(map[&2], "two");

        let translation_key = parse_translations(r#"[{"n": 1, "translation": "uno"}]"#, 1);
        assert_eq!(translation_key[&1], "uno");
    }

    /// A payload that starts like JSON but is not parseable falls through to the
    /// numbered-line reader instead of losing the whole answer.
    #[test]
    fn malformed_json_falls_back_to_numbered_lines() {
        let raw = "[1. Hello\n2. World";
        let parsed = parse_translations(raw, 1);

        assert_eq!(parsed[&2], "World");
    }

    /// A line whose first token is not a number must never be read as a cue, or
    /// a wrapped translation starting with digits would shift the numbering.
    #[test]
    fn lines_without_a_valid_separator_are_not_cues() {
        assert!(split_numbered_line("2024 was a year").is_none());
        assert!(split_numbered_line("1 5 apples").is_none());
        assert!(split_numbered_line("hello").is_none());
        assert_eq!(
            split_numbered_line("  7. seven"),
            Some((7, "seven".to_string()))
        );
        // Text on the same line after the separator.
        assert_eq!(
            split_numbered_line("2) with trailing space   "),
            Some((2, "with trailing space".to_string()))
        );
        // An empty first line is still a real cue.
        assert_eq!(split_numbered_line("3."), Some((3, String::new())));
    }
}
