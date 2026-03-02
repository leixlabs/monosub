use serde::{Deserialize, Serialize};
use std::time::Duration;

const GITHUB_RELEASES_API_LATEST: &str =
    "https://api.github.com/repos/0xkamalei/monosub/releases/latest";
const DEFAULT_RELEASES_URL: &str = "https://github.com/0xkamalei/monosub/releases";

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

#[tauri::command]
pub async fn check_github_release_update() -> Result<GithubReleaseUpdateResult, String> {
    let current_version = env!("CARGO_PKG_VERSION").to_string();
    let user_agent = format!("MonoSub/{} (+https://github.com/0xkamalei/monosub)", current_version);

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
        .map_err(|e| format!("Failed to query GitHub releases: {}", e))?;

    if !response.status().is_success() {
        return Err(format!(
            "GitHub API request failed with status {}",
            response.status()
        ));
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
        release_url: if release.html_url.trim().is_empty() {
            DEFAULT_RELEASES_URL.to_string()
        } else {
            release.html_url
        },
        published_at: release.published_at,
        has_update,
    })
}
