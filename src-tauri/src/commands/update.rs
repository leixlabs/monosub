use serde::{Deserialize, Serialize};
use std::time::Duration;

const GITHUB_RELEASES_API_LATEST: &str =
    "https://api.github.com/repos/leixlabs/monosub/releases/latest";
const DEFAULT_RELEASES_URL: &str = "https://github.com/leixlabs/monosub/releases";

#[derive(Debug, Deserialize)]
struct GithubLatestRelease {
    tag_name: String,
    html_url: String,
    name: Option<String>,
    published_at: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GithubReleaseUpdateResult {
    pub current_version: String,
    pub latest_version: String,
    pub latest_tag: String,
    pub release_name: Option<String>,
    pub release_url: String,
    pub published_at: Option<String>,
    pub has_update: bool,
}

fn normalize_version(version: &str) -> &str {
    version.trim().trim_start_matches(['v', 'V'])
}

/// A non-2xx from the releases API, said in terms the user can act on.
///
/// This string is rendered verbatim in Settings ▸ About under
/// `settings.about.checkFailed`, so it names the repository and tells the user
/// where the download link still is. Getting `404` here means the repo moved or
/// has no published release, which is not something the user can fix by
/// retrying the button.
fn describe_failed_response(status: reqwest::StatusCode) -> String {
    if status == reqwest::StatusCode::NOT_FOUND {
        format!(
            "No published release found at {} (HTTP 404). Download manually from {}",
            GITHUB_RELEASES_API_LATEST, DEFAULT_RELEASES_URL
        )
    } else {
        format!("{} returned HTTP {}", GITHUB_RELEASES_API_LATEST, status)
    }
}

fn has_newer_version(current_version: &str, latest_version: &str) -> bool {
    let current_normalized = normalize_version(current_version);
    let latest_normalized = normalize_version(latest_version);

    match (
        semver::Version::parse(current_normalized),
        semver::Version::parse(latest_normalized),
    ) {
        (Ok(current), Ok(latest)) => latest > current,
        _ => latest_normalized != current_normalized,
    }
}

/// The page the "Open Download Page" button opens. A release with no
/// `html_url` must still leave the user a working page, so this falls back to
/// the repository's releases list instead of an empty string.
fn release_url_or_default(html_url: &str) -> String {
    if html_url.trim().is_empty() {
        DEFAULT_RELEASES_URL.to_string()
    } else {
        html_url.to_string()
    }
}

#[tauri::command]
pub async fn check_github_release_update() -> Result<GithubReleaseUpdateResult, String> {
    let current_version = env!("CARGO_PKG_VERSION").to_string();
    let user_agent = format!(
        "MonoSub/{} (+https://github.com/leixlabs/monosub)",
        current_version
    );

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(12))
        .build()
        .map_err(|e| format!("Failed to initialize update client: {}", e))?;

    let response = client
        .get(GITHUB_RELEASES_API_LATEST)
        .header(reqwest::header::ACCEPT, "application/vnd.github+json")
        .header(reqwest::header::USER_AGENT, user_agent)
        .send()
        .await
        .map_err(|e| format!("Failed to query {}: {}", GITHUB_RELEASES_API_LATEST, e))?;

    if !response.status().is_success() {
        return Err(describe_failed_response(response.status()));
    }

    let release = response
        .json::<GithubLatestRelease>()
        .await
        .map_err(|e| format!("Invalid GitHub release response: {}", e))?;

    let latest_version = normalize_version(&release.tag_name).to_string();
    let has_update = has_newer_version(&current_version, &latest_version);

    Ok(GithubReleaseUpdateResult {
        current_version,
        latest_version,
        latest_tag: release.tag_name,
        release_name: release.name,
        release_url: release_url_or_default(&release.html_url),
        published_at: release.published_at,
        has_update,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The API host and the fallback page must name the same repository, or a
    /// successful check can still send the user to a dead download page.
    #[test]
    fn the_api_and_the_fallback_page_target_one_repository() {
        assert_eq!(
            GITHUB_RELEASES_API_LATEST,
            "https://api.github.com/repos/leixlabs/monosub/releases/latest"
        );
        assert_eq!(
            DEFAULT_RELEASES_URL,
            "https://github.com/leixlabs/monosub/releases"
        );
    }

    #[test]
    fn a_missing_release_names_the_repository_and_the_download_page() {
        let message = describe_failed_response(reqwest::StatusCode::NOT_FOUND);

        assert!(message.contains("leixlabs/monosub"), "got: {message}");
        assert!(message.contains("404"), "got: {message}");
        assert!(message.contains(DEFAULT_RELEASES_URL), "got: {message}");
    }

    #[test]
    fn other_failures_keep_the_status_and_the_endpoint() {
        let message = describe_failed_response(reqwest::StatusCode::FORBIDDEN);

        assert!(message.contains("403"), "got: {message}");
        assert!(message.contains(GITHUB_RELEASES_API_LATEST), "got: {message}");
    }

    #[test]
    fn an_empty_html_url_falls_back_to_the_releases_page() {
        assert_eq!(release_url_or_default(""), DEFAULT_RELEASES_URL);
        assert_eq!(release_url_or_default("   "), DEFAULT_RELEASES_URL);
        // A real value is passed through untouched.
        assert_eq!(
            release_url_or_default("https://github.com/leixlabs/monosub/releases/tag/v2.1.0"),
            "https://github.com/leixlabs/monosub/releases/tag/v2.1.0"
        );
    }

    #[test]
    fn version_comparison_still_prefers_semver_order() {
        assert!(has_newer_version("2.0.0", "v2.0.1"));
        assert!(!has_newer_version("2.0.0", "v2.0.0"));
        assert!(!has_newer_version("2.0.0", "1.9.9"));
    }
}
