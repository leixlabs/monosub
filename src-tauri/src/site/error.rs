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
    pub fn allows_fallback(&self) -> bool {
        matches!(
            self,
            ImportError::RateLimited | ImportError::SubtitleUnusable(_) | ImportError::Other(_)
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
        // and an unclassified failure. Not a login wall or a dead video.
        assert!(ImportError::RateLimited.allows_fallback());
        assert!(ImportError::SubtitleUnusable("empty".into()).allows_fallback());
        assert!(ImportError::Other("?".into()).allows_fallback());
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
}
