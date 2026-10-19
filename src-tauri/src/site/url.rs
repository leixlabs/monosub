//! `parse_url_input`: split, normalise, de-duplicate and whitelist the text the
//! user pasted into the URL import dialog.
//!
//! Task: U1-6. See docs/tech-spec/url-import-design.md §3 (the pure function in
//! front of every job) and §1 (site scope).
//!
//! This lives in Rust rather than in the frontend for one reason: it is the only
//! way the rules get unit tests (the frontend has no test runner), and a single
//! source of truth matters because `start_url_import` trusts its input.

use url::Url;

/// Where a pasted string came from, and what to tell the user when it is
/// rejected. `Ok` carries the normalised URL, which is what the backend then
/// hands to yt-dlp.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ParsedUrl {
    Ok {
        /// The string exactly as the user pasted it, so the UI can point at it.
        input: String,
        /// Normalised: tracking parameters stripped, `p` kept, https scheme.
        url: String,
    },
    Err {
        input: String,
        error: UrlError,
    },
}

/// Why a pasted string was rejected (url-import-design §5.9).
///
/// Serialised as a stable snake_case tag: the frontend maps each tag to an i18n
/// key, so these spellings are part of the IPC contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UrlError {
    /// A site this build does not support (YouTube, a random URL, not a URL).
    UnsupportedSite,
    /// A supported site, but a page kind that is out of scope: bangumi, cheese
    /// (paid courses), a space page or a favourites list.
    UnsupportedKind,
}

impl UrlError {
    /// Stable tag used by the IPC payload and by the i18n key the UI resolves.
    ///
    /// `serde` produces the same spelling (`rename_all = "snake_case"`); this
    /// method exists so a Rust caller never has to hard-code the string.
    #[cfg(test)]
    pub fn as_str(self) -> &'static str {
        match self {
            UrlError::UnsupportedSite => "unsupported_site",
            UrlError::UnsupportedKind => "unsupported_kind",
        }
    }
}

/// Query parameters that only carry tracking data. `p` is deliberately NOT in
/// this list: it selects the part of a multi-P video (design §4.4).
const TRACKING_PARAMS: [&str; 8] = [
    "spm_id_from",
    "vd_source",
    "from_source",
    "seid",
    "bbid",
    "ts",
    "unique_k",
    "share_source",
];

/// Hosts whose `/video/BV…` or `/video/av…` path is a single video.
const VIDEO_HOSTS: [&str; 3] = ["bilibili.com", "www.bilibili.com", "m.bilibili.com"];
/// Short-link host; yt-dlp's generic extractor follows the redirect itself.
const SHORT_HOST: &str = "b23.tv";
/// Every accepted video host is rewritten to this one before de-duplication.
const CANONICAL_VIDEO_HOST: &str = "www.bilibili.com";

/// Path prefixes that a supported site serves but this build refuses.
///
/// Bangumi and cheese are paid/DRM content, while space and favlist are lists
/// rather than a single video — the same reasoning as `--no-playlist` (design §1).
const REJECTED_PATHS: [(&str, &str); 4] = [
    ("/bangumi/", "bangumi"),
    ("/cheese/", "cheese"),
    ("/space/", "space"),
    ("/favlist", "favlist"),
];

/// Split the dialog's text on spaces, newlines and commas, then normalise and
/// de-duplicate.
///
/// Order is preserved (the UI shows one row per input) and duplicates are
/// collapsed *after* normalisation, so `…/BV1x?spm_id_from=a` and `…/BV1x` are
/// recognised as the same video.
pub fn parse_input(text: &str) -> Vec<ParsedUrl> {
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut parsed: Vec<ParsedUrl> = Vec::new();

    for raw in text.split([' ', '\t', '\n', '\r', ',', '，']) {
        let input = raw.trim();
        if input.is_empty() {
            continue;
        }

        match normalise(input) {
            Ok(url) => {
                if !seen.insert(url.clone()) {
                    tracing::debug!(url = %url, "duplicate URL dropped");
                    continue;
                }
                parsed.push(ParsedUrl::Ok {
                    input: input.to_string(),
                    url,
                });
            }
            Err(error) => {
                // Duplicate *invalid* inputs are reported once each time: the UI
                // marks the row the user typed, and two identical bad rows are
                // two rows to fix.
                parsed.push(ParsedUrl::Err {
                    input: input.to_string(),
                    error,
                });
            }
        }
    }

    parsed
}

/// A bare `av12345` / `BV1xx411c7mD` is what users paste from a chat message.
fn as_video_path(input: &str) -> Option<String> {
    let candidate = input.trim_start_matches('/');
    let is_av = candidate
        .strip_prefix("av")
        .is_some_and(|digits| !digits.is_empty() && digits.chars().all(|c| c.is_ascii_digit()));
    let is_bv = candidate.starts_with("BV") && candidate.len() >= 10;
    (is_av || is_bv).then(|| format!("/video/{}", candidate))
}

/// One pasted string → the URL the backend will use.
fn normalise(input: &str) -> Result<String, UrlError> {
    // A bare video id is a URL the user abbreviated; give it the canonical host.
    let candidate = match as_video_path(input) {
        Some(path) => format!("https://www.bilibili.com{}", path),
        None => {
            if input.starts_with("http://") || input.starts_with("https://") {
                input.to_string()
            } else if input.contains("://") {
                // Some other scheme (ftp:, magnet:) — not a page we can read.
                return Err(UrlError::UnsupportedSite);
            } else {
                format!("https://{}", input)
            }
        }
    };

    let mut url = Url::parse(&candidate).map_err(|_| UrlError::UnsupportedSite)?;
    let raw_host = url
        .host_str()
        .ok_or(UrlError::UnsupportedSite)?
        .to_ascii_lowercase();

    // The short link has no video path of its own: yt-dlp's generic extractor
    // follows the redirect, so the URL is passed through untouched. It is
    // checked before the `www.` trimming below, which would not match `b23.tv`.
    if raw_host == SHORT_HOST {
        url.set_fragment(None);
        return Ok(url.to_string());
    }

    if !VIDEO_HOSTS.contains(&raw_host.as_str()) {
        return Err(UrlError::UnsupportedSite);
    }

    // One canonical host, so `m.bilibili.com/video/BV1x` and
    // `www.bilibili.com/video/BV1x` de-duplicate to the same entry (the mobile
    // page and the desktop page are the same video).
    if url.set_host(Some(CANONICAL_VIDEO_HOST)).is_err() {
        return Err(UrlError::UnsupportedSite);
    }

    let path = url.path().to_string();
    if REJECTED_PATHS
        .iter()
        .any(|(prefix, _)| path.starts_with(prefix) || path == prefix.trim_end_matches('/'))
    {
        return Err(UrlError::UnsupportedKind);
    }

    // `/video/BV…` or `/video/av…` only. Anything else on a supported host
    // (a homepage, a `/read/` article) is out of scope.
    let video_path = path.strip_prefix("/video/").unwrap_or_default();
    if video_path.is_empty() {
        return Err(UrlError::UnsupportedKind);
    }
    if !(video_path.starts_with("BV") || video_path.starts_with("av")) {
        return Err(UrlError::UnsupportedKind);
    }

    // Rebuild the query keeping only what changes which video is meant.
    let kept: Vec<(String, String)> = url
        .query_pairs()
        .filter(|(key, _)| !TRACKING_PARAMS.contains(&key.as_ref()))
        .map(|(key, value)| (key.into_owned(), value.into_owned()))
        .collect();

    url.set_scheme("https").ok();
    url.set_fragment(None);
    if kept.is_empty() {
        url.set_query(None);
    } else {
        url.set_query(Some(
            &kept
                .iter()
                .map(|(key, value)| format!("{}={}", key, value))
                .collect::<Vec<_>>()
                .join("&"),
        ));
    }

    Ok(url.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ok(input: &str) -> Vec<String> {
        parse_input(input)
            .into_iter()
            .filter_map(|parsed| match parsed {
                ParsedUrl::Ok { url, .. } => Some(url),
                ParsedUrl::Err { .. } => None,
            })
            .collect()
    }

    fn errors(input: &str) -> Vec<(String, UrlError)> {
        parse_input(input)
            .into_iter()
            .filter_map(|parsed| match parsed {
                ParsedUrl::Err { input, error } => Some((input, error)),
                ParsedUrl::Ok { .. } => None,
            })
            .collect()
    }

    #[test]
    fn splits_on_every_documented_separator() {
        // Spaces, newlines, commas, tabs and the full-width comma.
        let text = "https://www.bilibili.com/video/BV1GJ411x7h7\n\
                    https://www.bilibili.com/video/BV1GJ411x7h8,\
                    https://www.bilibili.com/video/BV1GJ411x7h9，\
                    https://www.bilibili.com/video/BV1GJ411x7hA\thttps://www.bilibili.com/video/BV1GJ411x7hB";
        let urls = ok(text);
        assert_eq!(urls.len(), 5, "{urls:?}");
    }

    #[test]
    fn keeps_one_entry_per_input_in_order() {
        let parsed = parse_input(
            "https://www.bilibili.com/video/BV1GJ411x7h7 not-a-url \
             https://www.bilibili.com/video/BV1GJ411x7h8",
        );
        assert_eq!(parsed.len(), 3);
        assert!(matches!(parsed[0], ParsedUrl::Ok { .. }));
        assert!(matches!(parsed[1], ParsedUrl::Err { .. }));
        assert!(matches!(parsed[2], ParsedUrl::Ok { .. }));
    }

    #[test]
    fn deduplicates_after_normalising() {
        // Same video, three spellings: tracking prefix, mobile host, no scheme.
        let urls = ok("https://www.bilibili.com/video/BV1GJ411x7h7?spm_id_from=333.788 \
                       https://m.bilibili.com/video/BV1GJ411x7h7 \
                       www.bilibili.com/video/BV1GJ411x7h7");
        assert_eq!(urls.len(), 1, "{urls:?}");
        assert_eq!(urls[0], "https://www.bilibili.com/video/BV1GJ411x7h7");
    }

    #[test]
    fn accepts_the_documented_hosts_and_id_forms() {
        for input in [
            "https://www.bilibili.com/video/BV1GJ411x7h7",
            "https://m.bilibili.com/video/BV1GJ411x7h7",
            "https://bilibili.com/video/BV1GJ411x7h7",
            "https://www.bilibili.com/video/av80433022",
            "bilibili.com/video/BV1GJ411x7h7",
        ] {
            assert_eq!(ok(input).len(), 1, "{input} should be accepted");
        }

        // A bare id pasted from a chat message.
        assert_eq!(
            ok("BV1GJ411x7h7"),
            vec!["https://www.bilibili.com/video/BV1GJ411x7h7"]
        );
        assert_eq!(
            ok("av80433022"),
            vec!["https://www.bilibili.com/video/av80433022"]
        );

        // The short link is passed through for yt-dlp to follow.
        assert_eq!(ok("https://b23.tv/abc123"), vec!["https://b23.tv/abc123"]);
    }

    #[test]
    fn keeps_the_part_selector_and_drops_tracking_parameters() {
        // `p` chooses the part of a multi-P video, so it must survive.
        assert_eq!(
            ok("https://www.bilibili.com/video/BV1GJ411x7h7?p=3"),
            vec!["https://www.bilibili.com/video/BV1GJ411x7h7?p=3"]
        );
        // Tracking-only query becomes no query at all.
        assert_eq!(
            ok("https://www.bilibili.com/video/BV1GJ411x7h7?spm_id_from=333.999&vd_source=abc"),
            vec!["https://www.bilibili.com/video/BV1GJ411x7h7"]
        );
        // Mixed: the part survives, the tracking does not.
        assert_eq!(
            ok("https://www.bilibili.com/video/BV1GJ411x7h7?p=2&spm_id_from=333.999"),
            vec!["https://www.bilibili.com/video/BV1GJ411x7h7?p=2"]
        );
    }

    #[test]
    fn rejects_other_sites() {
        let cases = [
            "https://www.youtube.com/watch?v=dQw4w9WgXcQ",
            "https://youtu.be/dQw4w9WgXcQ",
            "https://example.com/video/BV1GJ411x7h7",
            "not-a-url",
            "ftp://www.bilibili.com/video/BV1GJ411x7h7",
        ];
        for input in cases {
            let errors = errors(input);
            assert_eq!(errors.len(), 1, "{input} should be rejected");
            assert_eq!(errors[0].1, UrlError::UnsupportedSite, "{input}");
            assert_eq!(errors[0].1.as_str(), "unsupported_site");
        }
    }

    #[test]
    fn rejects_the_out_of_scope_page_kinds() {
        // A *space* page lives on its own subdomain, so the host whitelist
        // rejects it before the path rules ever run; that is still a useful,
        // separate message, so the test pins which one the user sees.
        let cases = [
            ("https://www.bilibili.com/bangumi/play/ep123456", UrlError::UnsupportedKind),
            ("https://www.bilibili.com/cheese/play/ep123456", UrlError::UnsupportedKind),
            ("https://space.bilibili.com/123456", UrlError::UnsupportedSite),
            ("https://www.bilibili.com/space/123456", UrlError::UnsupportedKind),
            ("https://www.bilibili.com/favlist?fid=123", UrlError::UnsupportedKind),
            // A supported host but not a video page.
            ("https://www.bilibili.com/", UrlError::UnsupportedKind),
            ("https://www.bilibili.com/read/cv123456", UrlError::UnsupportedKind),
        ];
        for (input, expected) in cases {
            let errors = errors(input);
            assert_eq!(errors.len(), 1, "{input} should be rejected");
            assert_eq!(errors[0].1, expected, "{input}");
        }
        assert_eq!(UrlError::UnsupportedKind.as_str(), "unsupported_kind");
    }

    #[test]
    fn blank_input_yields_nothing() {
        assert!(parse_input("").is_empty());
        assert!(parse_input("   \n\t , ， ").is_empty());
    }

    #[test]
    fn error_payload_is_a_stable_snake_case_tag() {
        // The IPC contract: the frontend keys `errors.urlImport.<tag>` on this.
        let json = serde_json::to_string(&UrlError::UnsupportedSite).unwrap();
        assert_eq!(json, "\"unsupported_site\"");
        let json = serde_json::to_string(&UrlError::UnsupportedKind).unwrap();
        assert_eq!(json, "\"unsupported_kind\"");

        // ...and the whole payload round-trips the way Tauri will send it.
        let parsed = parse_input("https://youtu.be/x");
        let json = serde_json::to_value(&parsed).unwrap();
        assert_eq!(json[0]["Err"]["error"], "unsupported_site");
        assert_eq!(json[0]["Err"]["input"], "https://youtu.be/x");
    }
}
