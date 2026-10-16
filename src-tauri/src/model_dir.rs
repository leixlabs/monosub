//! Where model files live, and the user's override for that location.
//!
//! Models are large (0.2–1.2 GB each), so re-downloading them after a reinstall,
//! a bundle-id change, or on a second machine is a real cost. The user can point
//! the app at an existing directory instead.
//!
//! Resolution order for the models root:
//!
//! 1. `MONOSUB_MODELS_DIR` environment override (dev/CI convenience).
//! 2. The persisted `modelsDir` from `<app_config_dir>/settings.json`.
//! 3. `<app_data_dir>/models` — the default, unchanged from previous versions.
//!
//! There is exactly one implementation so the catalogue (`engine::catalog`,
//! which decides `is_installed` and the `--model` path) and the downloader
//! (`commands::models`, which writes files) can never disagree about where the
//! models are.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tauri::Manager;

/// Env override, mirroring `MONOSUB_FFMPEG_PATH` / `MONOSUB_AUDIOCPP_PATH`.
pub const MODELS_DIR_ENV: &str = "MONOSUB_MODELS_DIR";

/// Settings file name inside `<app_config_dir>`.
const SETTINGS_FILE: &str = "settings.json";

/// The persisted slice of app settings that Rust owns.
///
/// Only `models_dir` lives here: the rest of the app's preferences are
/// `localStorage`-backed and never need to be read by the backend. Keeping this
/// minimal avoids a second source of truth for settings the frontend owns.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BackendSettings {
    /// User-chosen models directory. `None` means "use the default".
    #[serde(default, rename = "modelsDir", skip_serializing_if = "Option::is_none")]
    pub models_dir: Option<String>,
}

/// `<app_config_dir>/settings.json`.
pub fn settings_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let config_dir = app
        .path()
        .app_config_dir()
        .map_err(|e| format!("failed to resolve app config dir: {}", e))?;
    Ok(config_dir.join(SETTINGS_FILE))
}

/// Read the persisted backend settings; missing/corrupt files yield defaults.
///
/// A corrupt file must not brick the app: models are large, and refusing to
/// start because a settings JSON was truncated would be far worse than falling
/// back to the default location (which the downloaded-models flow re-creates).
pub fn load_settings(app: &tauri::AppHandle) -> BackendSettings {
    let Ok(path) = settings_path(app) else {
        return BackendSettings::default();
    };
    let Ok(raw) = std::fs::read_to_string(&path) else {
        return BackendSettings::default();
    };
    match serde_json::from_str::<BackendSettings>(&raw) {
        Ok(settings) => settings,
        Err(error) => {
            tracing::warn!(
                path = %path.display(),
                error = %error,
                "settings.json is not valid JSON; using defaults"
            );
            BackendSettings::default()
        }
    }
}

/// Persist the backend settings, creating `<app_config_dir>` when needed.
pub fn save_settings(app: &tauri::AppHandle, settings: &BackendSettings) -> Result<(), String> {
    let path = settings_path(app)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("failed to create {}: {}", parent.display(), e))?;
    }
    let body = serde_json::to_string_pretty(settings)
        .map_err(|e| format!("failed to serialize settings: {}", e))?;
    std::fs::write(&path, body).map_err(|e| format!("failed to write {}: {}", path.display(), e))
}

/// The configured models root, without checking that it exists.
///
/// Precedence: env override → persisted setting → `<app_data_dir>/models`.
pub fn configured_root(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    if let Some(raw) = std::env::var_os(MODELS_DIR_ENV) {
        let trimmed = raw.to_string_lossy().trim().to_string();
        if !trimmed.is_empty() {
            return Ok(PathBuf::from(trimmed));
        }
    }

    if let Some(configured) = load_settings(app).models_dir {
        let trimmed = configured.trim();
        if !trimmed.is_empty() {
            return Ok(PathBuf::from(trimmed));
        }
    }

    default_root(app)
}

/// `<app_data_dir>/models` — the location used when nothing overrides it.
pub fn default_root(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_data_dir()
        .map(|dir| dir.join("models"))
        .map_err(|e| format!("failed to resolve app data dir: {}", e))
}

/// Validate a user-supplied models directory.
///
/// The directory must already exist (the point is to point at models the user
/// already downloaded) and must be writable, because downloads and
/// `cancel_model_download` cleanups write here. An existing file is rejected.
pub fn validate_dir(path: &Path) -> Result<(), String> {
    if !path.exists() {
        return Err(format!("directory does not exist: {}", path.display()));
    }
    if !path.is_dir() {
        return Err(format!("not a directory: {}", path.display()));
    }

    // A writability probe is the only reliable check cross-platform; metadata
    // permissions lie under ACLs and SIP-protected trees.
    let probe = path.join(".monosub-write-probe");
    match std::fs::write(&probe, b"probe") {
        Ok(()) => {
            let _ = std::fs::remove_file(&probe);
            Ok(())
        }
        Err(error) => Err(format!("directory is not writable: {} ({})", path.display(), error)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("monosub-modelsdir-{}-{}", tag, std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create scratch dir");
        dir
    }

    #[test]
    fn validate_dir_accepts_a_writable_directory() {
        let dir = scratch("ok");
        assert!(validate_dir(&dir).is_ok());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn validate_dir_rejects_missing_paths_and_files() {
        let missing = scratch("missing").join("nope");
        assert!(validate_dir(&missing).is_err());

        let dir = scratch("file");
        let file = dir.join("a-file");
        std::fs::write(&file, b"x").unwrap();
        let err = validate_dir(&file).expect_err("a file is not a directory");
        assert!(err.contains("not a directory"), "{err}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn settings_round_trip_keeps_models_dir() {
        let dir = scratch("settings");
        let path = dir.join(SETTINGS_FILE);

        let settings = BackendSettings {
            models_dir: Some("/Volumes/Big/models".to_string()),
        };
        let body = serde_json::to_string_pretty(&settings).unwrap();
        std::fs::write(&path, body).unwrap();

        let raw = std::fs::read_to_string(&path).unwrap();
        let parsed: BackendSettings = serde_json::from_str(&raw).unwrap();
        assert_eq!(parsed.models_dir.as_deref(), Some("/Volumes/Big/models"));
        // The wire name is camelCase so the frontend can share the file shape.
        assert!(raw.contains("\"modelsDir\""));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn settings_defaults_on_corrupt_json() {
        // Mirrors `load_settings`'s fallback without needing an AppHandle.
        let raw = "{ not json";
        let parsed = serde_json::from_str::<BackendSettings>(raw);
        assert!(parsed.is_err(), "corrupt input must fail to parse");
        assert!(BackendSettings::default().models_dir.is_none());
    }

    #[test]
    fn omitted_models_dir_serializes_away() {
        let body = serde_json::to_string(&BackendSettings::default()).unwrap();
        assert!(!body.contains("modelsDir"), "None must not persist: {body}");
    }
}
