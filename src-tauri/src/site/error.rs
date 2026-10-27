//! `ImportError` and yt-dlp stderr classification.
//!
//! Task: U1-11. See docs/tech-spec/url-import-design.md §4.5 and §5.9.
//!
//! Matching on stderr text is the one place this feature parses prose, and it is
//! unavoidable: yt-dlp is not localised and exposes no structured error channel
//! on the failure path. Every matched string therefore lives here as a constant,
//! and a fixture test pins one stderr sample per class.

/// The sentinel the frontend routes on to show the browser picker (design §4.5).
pub const NEEDS_COOKIES_STATUS: &str = "URL_IMPORT_NEEDS_COOKIES";

/// yt-dlp's own field names, for the models that carry them.
pub mod stderr {
    /// Bilibili serves this when the request has no usable cookie (U0-9: it is
    /// intermittent风控 rather than deterministic, so it is a *possible* answer,
    /// not the only one).
    pub const PRECONDITION_FAILED: &str = "HTTP Error 412";
    /// Too many requests; worth one or two backoff retries (design §5.9).
    pub const RATE_LIMITED: &str = "HTTP Error 429";
    /// Emitted as a WARNING when the video has AI subtitles but the request is
    /// anonymous. Not an error: the run still falls back to transcription.
    pub const NEEDS_LOGIN_SUBTITLES: &str = "Subtitles are only available when logged in";
    /// Private / deleted / region-blocked videos.
    /// Deliberately specific. `Unable to download webpage` is NOT in this list:
    /// it wraps every failure (including timeouts and 412), so matching it would
    /// classify a retryable network error as a dead video.
    pub const UNAVAILABLE_MARKERS: [&str; 6] = [
        "Private video",
        "This video is private",
        "Video unavailable",
        "video is unavailable",
        "This video has been removed",
        "This video is not available in your country",
    ];
    /// yt-dlp could not read the browser's cookie jar: the SQLite file was
    /// locked (Chrome/Edge running on Windows), the profile path did not exist,
    /// or Chrome's app-bound encryption defeated the decryption. Cookies are
    /// optional for every stage, so this is only fatal if the retry without them
    /// also fails.
    pub const COOKIE_DB_UNREADABLE: &str = "cookie database";
    /// Network-level failures: worth a retry, never worth a login prompt.
    pub const NETWORK_MARKERS: [&str; 5] = [
        "Connection reset by peer",
        "timed out",
        "Temporary failure in name resolution",
        "Failed to resolve",
        "Network is unreachable",
    ];
}

/// Why a URL import failed, in terms the UI can act on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportError {
    /// Show the browser picker; the user must supply cookies to continue.
    NeedsCookies,
    /// Private, deleted or region-locked: retrying changes nothing.
    Unavailable(String),
    /// The URL points at a multi-part video without a part selector.
    MultiPart,
    /// Connection/timeout: retryable.
    Network(String),
    /// HTTP 429: back off and retry; the caller degrades after the retries.
    RateLimited,
    /// The bundled executable is missing or not executable.
    BinaryMissing(String),
    /// The configured cookie source could not be read, so the request went out
    /// anonymously and the site asked for a login/bot check.
    CookiesUnreadable(String),
    /// A subtitle could not be parsed or was empty.
    SubtitleUnusable(String),
    /// Anything else, with the stderr tail for the log.
    Other(String),
}

impl std::fmt::Display for ImportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.status())
    }
}

impl std::error::Error for ImportError {}

impl ImportError {
    /// What `url_import_error` carries. Sentinels and i18n keys stay stable; a
    /// free-form message is shown verbatim by the frontend.
    pub fn status(&self) -> String {
        match self {
            ImportError::NeedsCookies => NEEDS_COOKIES_STATUS.to_string(),
            ImportError::Unavailable(detail) => format!("urlImport.unavailable:{}", detail),
            ImportError::MultiPart => "urlImport.multiPart".to_string(),
            ImportError::Network(detail) => format!("urlImport.network:{}", detail),
            ImportError::RateLimited => "urlImport.rateLimited".to_string(),
            ImportError::CookiesUnreadable(detail) => {
                format!("urlImport.cookiesUnreadable:{}", detail)
            }
            ImportError::BinaryMissing(detail) => format!("urlImport.engineMissing:{}", detail),
            ImportError::SubtitleUnusable(detail) => format!("urlImport.subtitleUnusable:{}", detail),
            ImportError::Other(detail) => detail.clone(),
        }
    }

    /// Whether retrying the same request could plausibly succeed.
    pub fn is_retryable(&self) -> bool {
        matches!(self, ImportError::RateLimited | ImportError::Network(_))
    }

    /// Whether falling back to transcription is a sane response. A login wall,
    /// a deleted video and a missing binary are not: the audio fetch would fail
    /// the same way, so the user would wait for nothing.
    ///
    /// An unreadable cookie jar is the exception among the auth failures: only
    /// some sites gate *subtitles* behind a login, while the audio is public, so
    /// one anonymous retry of the media path is still worth it (design §4.5).
    pub fn allows_fallback(&self) -> bool {
        matches!(
            self,
            ImportError::RateLimited
                | ImportError::SubtitleUnusable(_)
                | ImportError::CookiesUnreadable(_)
                | ImportError::Other(_)
        )
    }
}

/// Classify the stderr a yt-dlp invocation produced.
///
/// Precedence matters: 412 and 429 are both `HTTP Error`, but they need
/// different remediation, and `NeedsCookies` must win over the generic
/// `Unavailable` wording that may appear on the same line.
pub fn classify(stderr: &str) -> ImportError {
    let text = stderr;

    if text.contains(stderr::PRECONDITION_FAILED) {
        return ImportError::NeedsCookies;
    }
    if text.contains(stderr::RATE_LIMITED) {
        return ImportError::RateLimited;
    }
    if let Some(marker) = stderr::UNAVAILABLE_MARKERS.iter().find(|marker| text.contains(**marker)) {
        return ImportError::Unavailable(short_reason(text, marker));
    }
    if let Some(marker) = stderr::NETWORK_MARKERS.iter().find(|marker| text.contains(**marker)) {
        return ImportError::Network(short_reason(text, marker));
    }

    ImportError::Other(last_error_line(text))
}

/// Whether the run should still fall back to transcription but tell the user that
/// logging in would have given them the platform subtitle (design §4.5).
pub fn needs_login_hint(stderr: &str) -> bool {
    stderr.contains(stderr::NEEDS_LOGIN_SUBTITLES)
}

/// Whether yt-dlp aborted because it could not read the browser's cookie jar.
///
/// Checked *before* classifying, because the fix is not "log in" or "the video is
/// gone" but "retry the same stage without `--cookies-from-browser`": a locked
/// Chrome database (the common Windows case, yt-dlp#7271) or Chrome's app-bound
/// encryption breaks cookie *reading*, yet the subtitles of an ordinary public
/// video never needed cookies in the first place. Treating it as a hard failure
/// aborts an import that would otherwise have succeeded.
///
/// The quoted yt-dlp wording is matched case-insensitively and paired with a
/// qualifier, so an unrelated `PermissionError` (say, an unwritable output path)
/// does not silently drop the user's cookies.
pub fn is_cookie_database_failure(stderr: &str) -> bool {
    let text = stderr.to_ascii_lowercase();
    let names_the_db = text.contains(stderr::COOKIE_DB_UNREADABLE)
        || text.contains("cookies database")
        // Chrome 127+ app-bound encryption: the copy succeeds, the decryption
        // does not, and yt-dlp names DPAPI rather than the database.
        || text.contains("failed to decrypt with dpapi")
        || (text.contains("cookie") && text.contains("failed to decrypt"));

    if !names_the_db {
        return false;
    }

    // `Could not copy …`, `could not find chrome cookies database in …` and the
    // decrypt path are all "the jars were unreadable".
    text.contains("could not") || text.contains("unable to") || text.contains("failed to")
}

/// The `ERROR: …` line if there is one, else the last non-empty line. Used so a
/// user-facing message is the actual complaint rather than the whole banner.
fn last_error_line(stderr: &str) -> String {
    let lines: Vec<&str> = stderr
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect();

    if let Some(error) = lines.iter().rev().find(|line| line.starts_with("ERROR:")) {
        return error.trim_start_matches("ERROR:").trim().to_string();
    }
    lines.last().map(|line| (*line).to_string()).unwrap_or_default()
}

/// One short phrase around the matched marker, for a message that still says
/// *which* unavailable it was.
fn short_reason(text: &str, marker: &str) -> String {
    text.lines()
        .find(|line| line.contains(marker))
        .map(|line| line.trim().trim_start_matches("ERROR:").trim().to_string())
        .unwrap_or_else(|| marker.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One fixture per class, written from the shapes U0-9 observed. The point is
    /// that every class has a *representative* sample, so a rewrite of the
    /// matching rules cannot silently start classifying everything as `Other`.
    const NEEDS_COOKIES: &str = "\
ERROR: [BiliBili] 1GJ411x7h7: Unable to download JSON metadata: HTTP Error 412: Precondition Failed";

    const RATE_LIMITED: &str = "\
WARNING: [BiliBili] rate limited
ERROR: unable to download video subtitles: HTTP Error 429: Too Many Requests";

    const UNAVAILABLE: &str = "\
[BiliBili] BV1xxxxxxxxx: Downloading webpage
ERROR: [BiliBili] BV1xxxxxxxxx: Video unavailable";

    const NETWORK: &str = "\
ERROR: Unable to download webpage: <urlopen error [Errno 60] Operation timed out>";

    const LOGIN_HINT: &str = "\
WARNING: [BiliBili] Subtitles are only available when logged in. Use --cookies-from-browser or --cookies for the authentication.";

    #[test]
    fn classifies_each_documented_failure_kind() {
        assert_eq!(classify(NEEDS_COOKIES), ImportError::NeedsCookies);
        assert_eq!(classify(RATE_LIMITED), ImportError::RateLimited);

        assert!(matches!(classify(UNAVAILABLE), ImportError::Unavailable(_)));
        assert!(matches!(classify(NETWORK), ImportError::Network(_)));

        let other = classify("ERROR: something unexpected happened\n");
        assert!(matches!(other, ImportError::Other(_)), "{other:?}");
    }

    #[test]
    fn needs_cookies_wins_over_the_unavailable_wording() {
        // A 412 body can also carry "Unable to download webpage" (an
        // `UNAVAILABLE_MARKERS` entry), but the remediation is cookies.
        let mixed = "ERROR: [BiliBili] x: Unable to download webpage: HTTP Error 412: Precondition Failed";
        assert_eq!(classify(mixed), ImportError::NeedsCookies);
    }

    #[test]
    fn rate_limit_wins_over_the_generic_network_markers() {
        let mixed = "ERROR: timed out while downloading subtitles: HTTP Error 429: Too Many Requests";
        assert_eq!(classify(mixed), ImportError::RateLimited);
    }

    /// Chrome's cookie database locked (`yt-dlp#7271`) or made unreadable by its
    /// app-bound encryption must be recognised, because the fix is a retry
    /// without cookies rather than a login prompt.
    #[test]
    fn recognises_an_unreadable_browser_cookie_jar() {
        for stderr in [
            "ERROR: Could not copy Chrome cookie database. See \
             https://github.com/yt-dlp/yt-dlp/issues/7271 for more info",
            "ERROR: Could not copy Edge cookie database. See \
             https://github.com/yt-dlp/yt-dlp/issues/7271 for more info",
            "ERROR: could not find firefox cookies database in \
             \"C:\\Users\\x\\AppData\\Roaming\\Mozilla\\Firefox\\Profiles\\x\"",
            "ERROR: Failed to decrypt with DPAPI. See \
             https://github.com/yt-dlp/yt-dlp/issues/10927 for more info",
        ] {
            assert!(is_cookie_database_failure(stderr), "{stderr}");
        }

        for stderr in [
            "",
            "ERROR: [BiliBili] BV1x: HTTP Error 412: Precondition Failed",
            // A permission failure that has nothing to do with the cookie jar.
            "ERROR: unable to open output file: Permission denied",
            // Mentions cookies but the run was fine.
            "WARNING: Extracting cookies from chrome succeeded",
        ] {
            assert!(!is_cookie_database_failure(stderr), "{stderr}");
        }
    }

    #[test]
    fn detects_the_needs_login_subtitle_warning() {
        assert!(needs_login_hint(LOGIN_HINT));
        assert!(!needs_login_hint(""));
        assert!(!needs_login_hint("ERROR: HTTP Error 412"));
        // The phrase is part of the constant, so a partial match must not fire.
        assert!(!needs_login_hint("Subtitles are available"));
    }

    #[test]
    fn status_strings_are_stable_sentinels() {
        assert_eq!(ImportError::NeedsCookies.status(), NEEDS_COOKIES_STATUS);
        assert_eq!(NEEDS_COOKIES_STATUS, "URL_IMPORT_NEEDS_COOKIES");
        assert_eq!(ImportError::MultiPart.status(), "urlImport.multiPart");
        assert_eq!(ImportError::RateLimited.status(), "urlImport.rateLimited");
        assert_eq!(
            ImportError::Unavailable("gone".into()).status(),
            "urlImport.unavailable:gone"
        );
        assert_eq!(
            ImportError::BinaryMissing("no yt-dlp".into()).status(),
            "urlImport.engineMissing:no yt-dlp"
        );
        assert_eq!(
            ImportError::CookiesUnreadable("bot check".into()).status(),
            "urlImport.cookiesUnreadable:bot check"
        );
        // `Other` is a message, shown verbatim.
        assert_eq!(ImportError::Other("boom".into()).status(), "boom");
    }

    #[test]
    fn retry_and_fallback_policy_matches_the_design() {
        // Retryable: only the two transient kinds.
        assert!(ImportError::RateLimited.is_retryable());
        assert!(ImportError::Network("timeout".into()).is_retryable());
        assert!(!ImportError::NeedsCookies.is_retryable());
        assert!(!ImportError::Unavailable("gone".into()).is_retryable());
        assert!(!ImportError::MultiPart.is_retryable());
        assert!(!ImportError::BinaryMissing("x".into()).is_retryable());

        // Fallback to transcription: 429 (design §5.9), an unusable subtitle,
        // an unreadable cookie jar (the media may still be public) and an
        // unclassified failure. Not a login wall or a dead video.
        assert!(ImportError::RateLimited.allows_fallback());
        assert!(ImportError::SubtitleUnusable("empty".into()).allows_fallback());
        assert!(ImportError::CookiesUnreadable("bot check".into()).allows_fallback());
        assert!(ImportError::Other("?".into()).allows_fallback());
        // A cookie jar that cannot be read is a configuration problem, so a
        // plain retry of the same request changes nothing.
        assert!(!ImportError::CookiesUnreadable("x".into()).is_retryable());
        assert!(!ImportError::NeedsCookies.allows_fallback());
        assert!(!ImportError::Unavailable("gone".into()).allows_fallback());
        assert!(!ImportError::MultiPart.allows_fallback());
        assert!(!ImportError::BinaryMissing("x".into()).allows_fallback());
    }

    #[test]
    fn other_reports_the_error_line_not_the_banner() {
        let stderr = "\
[BiliBili] Extracting URL: https://www.bilibili.com/video/BV1x
[BiliBili] Downloading webpage
ERROR: [BiliBili] BV1x: something specific went wrong";
        match classify(stderr) {
            ImportError::Other(message) => {
                assert!(message.contains("something specific went wrong"), "{message}");
                assert!(!message.starts_with("ERROR:"), "{message}");
                assert!(!message.contains("Downloading webpage"), "{message}");
            }
            other => panic!("expected Other, got {other:?}"),
        }

        // No ERROR: line at all → the last non-empty line.
        match classify("step 1\nlast line\n\n") {
            ImportError::Other(message) => assert_eq!(message, "last line"),
            other => panic!("expected Other, got {other:?}"),
        }
        // Empty stderr must not panic.
        assert_eq!(classify(""), ImportError::Other(String::new()));
    }

    #[test]
    fn unavailable_message_names_the_reason() {
        let stderr = "ERROR: [BiliBili] BV1x: This video is private";
        match classify(stderr) {
            ImportError::Unavailable(detail) => assert!(detail.contains("private"), "{detail}"),
            other => panic!("expected Unavailable, got {other:?}"),
        }
    }

    /// The import statuses are i18n keys, not sentences: the frontend rewrites
    /// `urlImport.<name>` to `urlImport.errors.<name>` and falls back to the raw
    /// string. Pin every emitted name against the locale tree, so a rename
    /// cannot silently show the raw key again (exactly what hid the
    /// cookiesUnreadable message for weeks).
    #[test]
    fn import_statuses_emit_i18n_keys_that_exist_in_the_locales() {
        let cases = [
            ImportError::Unavailable("gone".into()),
            ImportError::MultiPart,
            ImportError::Network("timeout".into()),
            ImportError::RateLimited,
            ImportError::CookiesUnreadable("bot check".into()),
            ImportError::BinaryMissing("no yt-dlp".into()),
            ImportError::SubtitleUnusable("empty".into()),
        ];

        let locales = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("src/i18n/locales");

        for err in cases {
            let status = err.status();
            let key = status.split(':').next().unwrap();
            let name = key.strip_prefix("urlImport.").expect("status prefix");

            for entry in std::fs::read_dir(&locales).expect("read locales") {
                let path = entry.expect("entry").path();
                if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
                    continue;
                }
                let body = std::fs::read_to_string(&path).expect("read locale");
                let json: serde_json::Value = serde_json::from_str(&body)
                    .unwrap_or_else(|error| panic!("{}: {}", path.display(), error));
                assert!(
                    json.get("urlImport")
                        .and_then(|u| u.get("errors"))
                        .and_then(|e| e.get(name))
                        .is_some(),
                    "{} is missing urlImport.errors.{}",
                    path.display(),
                    name
                );
            }
        }
    }
}
