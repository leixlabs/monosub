//! Clipboard watch: decide whether the text the user just copied is a single
//! Bilibili video worth offering to import.
//!
//! Why this lives in Rust rather than in the polling frontend: the rule has
//! edges (a URL pasted from a chat message, a two-link batch, tracking
//! parameters, a YouTube link) and the frontend has no test runner. The frontend
//! only polls and renders; every judgement is made here and covered by tests.
//!
//! The bar is deliberately **high precision**: a prompt that appears for the
//! wrong reason is worse than a prompt that does not appear, because the user
//! learns to dismiss it without reading. So the text must contain exactly one
//! supported video URL, and it must be short enough to plausibly *be* a link
//! rather than a document that happens to mention one.

use crate::site::url::{parse_input, ParsedUrl};

/// Longest clipboard text considered. A copied link is tens of characters; a
/// copied article or log is far longer, and prompting for a URL buried in one
/// would be noise.
pub const MAX_CLIPBOARD_CHARS: usize = 500;

/// A single Bilibili video the user could import with one click.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ClipboardHit {
    /// Normalised URL, ready to hand to `start_url_import`.
    pub url: String,
    /// The video id (`BV1GJ411x7h7`), shown as the label before any network call
    /// tells us the title.
    ///
    /// `None` for a `b23.tv` short link: its path segment is an opaque short code
    /// (`xyz789`), not a video id, and showing it would look like a real id and
    /// be wrong. The UI then labels the pill with the shortened link instead.
    pub id: Option<String>,
    /// The text exactly as it was copied, so the UI can show what it acted on and
    /// so a dismissal can be remembered against it.
    pub text: String,
}

/// Classify clipboard text.
///
/// Returns `Some` only when the text holds **exactly one** supported video URL.
/// Zero is "nothing to offer"; two or more is a batch, which the paste dialog
/// handles better than a one-click prompt.
pub fn classify(text: &str) -> Option<ClipboardHit> {
    let trimmed = text.trim();
    if trimmed.is_empty() || trimmed.chars().count() > MAX_CLIPBOARD_CHARS {
        return None;
    }

    let parsed = parse_input(trimmed);
    let mut urls = parsed.iter().filter_map(|row| match row {
        ParsedUrl::Ok { url, .. } => Some(url.clone()),
        ParsedUrl::Err { .. } => None,
    });

    let url = urls.next()?;
    if urls.next().is_some() {
        // Two or more videos: this is a batch paste, not a single link.
        tracing::debug!("clipboard holds several URLs; not prompting");
        return None;
    }

    Some(ClipboardHit {
        id: video_id(&url),
        url,
        text: trimmed.to_string(),
    })
}

/// The id segment of a normalised `/video/<id>` URL, when the URL names a video
/// id directly.
///
/// A `b23.tv` short link is deliberately `None`: `xyz789` is a short code, not a
/// video id, and presenting it as one would be a small lie the user cannot check
/// until the import finishes (by which time it says `BV…` instead).
fn video_id(url: &str) -> Option<String> {
    if is_short_link(url) {
        return None;
    }

    let segment = url
        .split('/')
        .next_back()
        .map(|tail| tail.split(['?', '#']).next().unwrap_or(tail))
        .filter(|id| !id.is_empty())?;

    // Only a Bilibili video id is shown as an id. Anything else (an `av` number is
    // fine too) still names the video, so it is kept.
    Some(segment.to_string())
}

/// Whether this is a `b23.tv` short link, whose path is an opaque code.
fn is_short_link(url: &str) -> bool {
    ::url::Url::parse(url)
        .ok()
        .and_then(|parsed| parsed.host_str().map(str::to_ascii_lowercase))
        .is_some_and(|host| host == "b23.tv")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_plain_desktop_link_is_a_hit() {
        let hit = classify("https://www.bilibili.com/video/BV1GJ411x7h7").unwrap();
        assert_eq!(hit.url, "https://www.bilibili.com/video/BV1GJ411x7h7");
        assert_eq!(hit.id.as_deref(), Some("BV1GJ411x7h7"));
        assert_eq!(hit.text, "https://www.bilibili.com/video/BV1GJ411x7h7");
    }

    #[test]
    fn a_mobile_link_or_short_link_is_a_hit() {
        assert_eq!(
            classify("https://m.bilibili.com/video/BV1GJ411x7h7").unwrap().url,
            "https://www.bilibili.com/video/BV1GJ411x7h7",
            "the mobile host is canonicalised, so the same video dedupes"
        );
        assert_eq!(
            classify("https://b23.tv/abc123").unwrap().url,
            "https://b23.tv/abc123",
            "a short link is passed through for yt-dlp to follow"
        );
        assert_eq!(classify("BV1GJ411x7h7").unwrap().id.as_deref(), Some("BV1GJ411x7h7"));
        assert_eq!(classify("av80433022").unwrap().id.as_deref(), Some("av80433022"));
    }

    #[test]
    fn a_link_copied_out_of_a_sentence_is_a_hit() {
        // The realistic case: copied from a chat message.
        let hit = classify("看这个 https://www.bilibili.com/video/BV1GJ411x7h7 很好笑").unwrap();
        assert_eq!(hit.id.as_deref(), Some("BV1GJ411x7h7"));
        assert_eq!(
            hit.text,
            "看这个 https://www.bilibili.com/video/BV1GJ411x7h7 很好笑",
            "the stored text is what was copied, so dismissal can be remembered"
        );
    }

    #[test]
    fn tracking_parameters_are_stripped_before_the_hit_is_offered() {
        let hit = classify("https://www.bilibili.com/video/BV1GJ411x7h7?spm_id_from=333.999&vd_source=x").unwrap();
        assert_eq!(hit.url, "https://www.bilibili.com/video/BV1GJ411x7h7");
        // A part selector must survive, or the import would fetch the wrong part.
        assert_eq!(
            classify("https://www.bilibili.com/video/BV1G5po6zECY?p=3").unwrap().url,
            "https://www.bilibili.com/video/BV1G5po6zECY?p=3"
        );
    }

    #[test]
    fn a_batch_of_links_does_not_prompt() {
        // Two videos: the paste dialog is the right surface, not a one-click pill.
        assert!(classify(
            "https://www.bilibili.com/video/BV1GJ411x7h7 https://www.bilibili.com/video/BV1GJ411x7h8"
        )
        .is_none());

        // The same link twice is one video, so it still prompts.
        assert!(classify(
            "https://www.bilibili.com/video/BV1GJ411x7h7 https://m.bilibili.com/video/BV1GJ411x7h7"
        )
        .is_some());
    }

    #[test]
    fn unsupported_or_out_of_scope_links_do_not_prompt() {
        for text in [
            "https://www.youtube.com/watch?v=dQw4w9WgXcQ",
            "https://youtu.be/dQw4w9WgXcQ",
            "https://www.bilibili.com/bangumi/play/ep123456",
            "https://www.bilibili.com/cheese/play/ep123456",
            "https://www.bilibili.com/",
            "https://example.com/video/BV1GJ411x7h7",
        ] {
            assert!(classify(text).is_none(), "{text} must not prompt");
        }
    }

    #[test]
    fn ordinary_text_and_empty_clipboard_do_not_prompt() {
        for text in [
            "",
            "   ",
            "\n\n",
            "hello world",
            "这是一段普通文字，没有链接",
            "12345",
        ] {
            assert!(classify(text).is_none(), "{text:?} must not prompt");
        }
    }

    #[test]
    fn a_very_long_text_does_not_prompt_even_if_it_contains_a_link() {
        // A copied article or log that happens to mention a video must not
        // produce a prompt: the user did not copy a link, they copied a document.
        let long = format!(
            "{} https://www.bilibili.com/video/BV1GJ411x7h7",
            "字".repeat(MAX_CLIPBOARD_CHARS)
        );
        assert!(long.chars().count() > MAX_CLIPBOARD_CHARS);
        assert!(classify(&long).is_none());

        // ...while a text just under the limit still prompts. The padding is
        // derived from the URL length rather than guessed, so this cannot rot
        // when the fixture URL changes.
        let url = "https://www.bilibili.com/video/BV1GJ411x7h7";
        // `+ 1` for the space that separates the padding from the link:
        // `parse_input` splits on whitespace, so without it the link would be
        // glued to the padding and would not be a token of its own.
        let padding = MAX_CLIPBOARD_CHARS - url.chars().count() - 1;
        let just_under = format!("{} {}", "字".repeat(padding), url);
        assert_eq!(just_under.chars().count(), MAX_CLIPBOARD_CHARS);
        assert!(classify(&just_under).is_some(), "at the limit still prompts");
    }

    #[test]
    fn surrounding_whitespace_is_ignored() {
        let hit = classify("\n\n  https://www.bilibili.com/video/BV1GJ411x7h7  \n").unwrap();
        assert_eq!(hit.text, "https://www.bilibili.com/video/BV1GJ411x7h7");
        assert_eq!(hit.url, "https://www.bilibili.com/video/BV1GJ411x7h7");
    }

    /// Documented limitation, pinned so it cannot change by accident: the text is
    /// tokenised on whitespace, commas and tabs, so a URL glued directly to CJK
    /// text is not recognised. This is the high-precision trade-off — the
    /// alternative is a fuzzy substring scan, which would also "find" a link
    /// inside a log line and prompt for a video the user never copied.
    ///
    /// Copied share text always separates the link (Bilibili's own share sheet
    /// puts it on its own line), so the practical case is covered.
    #[test]
    fn a_url_glued_to_cjk_text_is_not_recognised_and_that_is_deliberate() {
        assert!(
            classify("看这个https://www.bilibili.com/video/BV1GJ411x7h7很好笑").is_none(),
            "a glued link must not produce a fuzzy match"
        );
        // One space on either side is enough.
        assert!(classify("看这个 https://www.bilibili.com/video/BV1GJ411x7h7 很好笑").is_some());
    }

    #[test]
    fn the_id_is_read_from_the_path_even_with_a_part_selector() {
        assert_eq!(
            video_id("https://www.bilibili.com/video/BV1G5po6zECY?p=3").as_deref(),
            Some("BV1G5po6zECY")
        );
        assert_eq!(
            video_id("https://www.bilibili.com/video/BV1GJ411x7h7").as_deref(),
            Some("BV1GJ411x7h7")
        );
        // Total, never panicking, on an unexpected shape.
        assert_eq!(video_id("BV1x").as_deref(), Some("BV1x"));
    }

    /// A short link's path is an opaque code, not a video id, so the pill must not
    /// label itself with it — the user cannot tell `xyz789` from a real id, and
    /// the import will report a `BV…` instead.
    #[test]
    fn a_short_link_has_no_id_to_show() {
        let hit = classify("https://b23.tv/xyz789").unwrap();
        assert_eq!(hit.url, "https://b23.tv/xyz789");
        assert_eq!(hit.id, None, "xyz789 is not a video id");

        // A real video URL keeps its id.
        assert!(classify("https://www.bilibili.com/video/BV1GJ411x7h7").unwrap().id.is_some());
    }

    /// The payload the IPC command returns is what the frontend renders, so its
    /// field names are part of the contract.
    #[test]
    fn the_hit_serialises_with_stable_field_names() {
        let hit = classify("https://www.bilibili.com/video/BV1GJ411x7h7").unwrap();
        let json = serde_json::to_value(&hit).unwrap();
        assert_eq!(json["url"], "https://www.bilibili.com/video/BV1GJ411x7h7");
        assert_eq!(json["id"], "BV1GJ411x7h7");
        // A short link serialises the absence explicitly rather than as a fake id.
        let short = classify("https://b23.tv/xyz789").unwrap();
        assert_eq!(serde_json::to_value(&short).unwrap()["id"], serde_json::Value::Null);
        assert_eq!(json["text"], "https://www.bilibili.com/video/BV1GJ411x7h7");
    }
}
