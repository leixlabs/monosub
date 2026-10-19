//! File naming for URL imports: `"<sanitized title> [<id>].<ext>"`.
//!
//! Task: U1-9. See docs/tech-spec/url-import-design.md §5.5.
//!
//! The id in the name is what makes it unique and idempotent: two different
//! videos cannot collide, re-importing the same URL produces the same path, and
//! therefore `skipExistingSubtitle` behaves exactly as it does for local files.

use std::path::{Path, PathBuf};

/// Characters Windows/macOS reject in a file name, plus the path separators.
const ILLEGAL: [char; 9] = ['/', '\\', ':', '*', '?', '"', '<', '>', '|'];
/// Longest title kept, counted in **characters** (see [`sanitize_title`]).
const MAX_TITLE_CHARS: usize = 120;

/// Turn a platform title into a file-name-safe stem.
///
/// Rules (design §5.5):
/// * `/ \ : * ? " < > |` and control characters become `_`;
/// * runs of whitespace collapse to one space;
/// * leading/trailing spaces and dots are removed (macOS hides dot-files, and a
///   trailing dot is invalid on Windows);
/// * the result is truncated to [`MAX_TITLE_CHARS`] **characters**, never bytes,
///   so a multi-byte character is never cut in half;
/// * an empty result becomes `""` — the caller supplies `[<id>]` instead.
pub fn sanitize_title(title: &str) -> String {
    let mut replaced = String::with_capacity(title.len());
    for ch in title.chars() {
        if ILLEGAL.contains(&ch) {
            replaced.push('_');
        } else if ch.is_whitespace() {
            // Tabs and newlines are separators, not content: keep the word break
            // and let the collapse below reduce them to one space.
            replaced.push(' ');
        } else if ch.is_control() {
            // A NUL or another non-whitespace control character carries no
            // usable text, so it becomes a plain separator-like underscore.
            replaced.push('_');
        } else {
            replaced.push(ch);
        }
    }

    // Collapse whitespace runs, then trim the edges (including dots).
    let collapsed = replaced.split_whitespace().collect::<Vec<_>>().join(" ");
    let trimmed = collapsed.trim_matches(|ch: char| ch == '.' || ch.is_whitespace());

    // Truncate by characters, not bytes: `chars().take(n)` cannot split one.
    trimmed.chars().take(MAX_TITLE_CHARS).collect::<String>()
        .trim_matches(|ch: char| ch == '.' || ch.is_whitespace())
        .to_string()
}

/// The file stem for one video: `<title> [<id>]`, or `[<id>]` when the title
/// sanitises away to nothing.
pub fn file_stem(title: &str, id: &str) -> String {
    let safe_title = sanitize_title(title);
    let safe_id = sanitize_title(id);
    if safe_title.is_empty() {
        format!("[{}]", safe_id)
    } else {
        format!("{} [{}]", safe_title, safe_id)
    }
}

/// The path to write for one format inside `dir`.
///
/// `first` decides whether this is the primary output (whose extension comes
/// from the format) or a later one — the same rule as
/// `subtitle::output_path_for`, expressed once here so the URL path does not
/// re-implement it.
pub fn output_path(dir: &Path, title: &str, id: &str, extension: &str) -> PathBuf {
    dir.join(format!("{}.{}", file_stem(title, id), extension))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replaces_every_illegal_character() {
        let sanitized = sanitize_title("a/b\\c:d*e?f\"g<h>i|j");
        assert_eq!(sanitized, "a_b_c_d_e_f_g_h_i_j");
        assert!(!sanitized.contains('/'));
    }

    #[test]
    fn replaces_control_characters() {
        let sanitized = sanitize_title("a\u{0}b\tc\nd");
        // A NUL is content-free and becomes `_`; tabs and newlines are word
        // breaks and collapse to one space.
        assert_eq!(sanitized, "a_b c d");
    }

    #[test]
    fn collapses_whitespace() {
        assert_eq!(sanitize_title("  a    b\t c  "), "a b c");
    }

    #[test]
    fn trims_leading_and_trailing_dots_and_spaces() {
        assert_eq!(sanitize_title("...hello..."), "hello");
        assert_eq!(sanitize_title("  . spaced .  "), "spaced");
        // Dots inside are preserved.
        assert_eq!(sanitize_title("v1.2 final"), "v1.2 final");
    }

    #[test]
    fn truncates_by_characters_without_splitting_multibyte() {
        // 130 CJK characters: 390 UTF-8 bytes, well past the 255-byte limit a
        // byte-wise cutoff would hit.
        let title = "字".repeat(130);
        let sanitized = sanitize_title(&title);
        assert_eq!(sanitized.chars().count(), MAX_TITLE_CHARS);
        assert_eq!(sanitized, "字".repeat(MAX_TITLE_CHARS));
        // Still a valid string (the assertion above would panic on invalid UTF-8).
        assert!(sanitized.chars().all(|ch| ch == '字'));

        // Mixed scripts, including a 4-byte emoji.
        let mixed = format!("{}{}", "a".repeat(119), "🎬🎬🎬🎬");
        let sanitized = sanitize_title(&mixed);
        assert_eq!(sanitized.chars().count(), MAX_TITLE_CHARS);
        assert!(sanitized.ends_with("🎬"));
    }

    #[test]
    fn an_emptied_title_does_not_leave_a_dangling_stem() {
        assert_eq!(sanitize_title("   "), "");
        assert_eq!(sanitize_title("..."), "");
        assert_eq!(sanitize_title("///"), "___");

        assert_eq!(file_stem("   ", "BV1GJ411x7h7"), "[BV1GJ411x7h7]");
        assert_eq!(file_stem("", "BV1GJ411x7h7"), "[BV1GJ411x7h7]");
    }

    #[test]
    fn stem_is_title_then_bracketed_id() {
        assert_eq!(
            file_stem("【官方 MV】Never Gonna Give You Up - Rick Astley", "BV1GJ411x7h7"),
            "【官方 MV】Never Gonna Give You Up - Rick Astley [BV1GJ411x7h7]"
        );
    }

    #[test]
    fn output_path_joins_dir_stem_and_extension() {
        let path = output_path(Path::new("/out"), "Title", "BV1", "srt");
        assert_eq!(path, PathBuf::from("/out/Title [BV1].srt"));

        let path = output_path(Path::new("/out"), "Title", "BV1", "vtt");
        assert_eq!(path, PathBuf::from("/out/Title [BV1].vtt"));

        // An id that itself carries illegal characters cannot break the path.
        let path = output_path(Path::new("/out"), "T", "BV/1", "srt");
        assert_eq!(path, PathBuf::from("/out/T [BV_1].srt"));
        assert_eq!(path.parent(), Some(Path::new("/out")));
    }
}
