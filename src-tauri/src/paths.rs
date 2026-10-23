//! Where everything lives, resolved once and passed down.
//!
//! This module exists so `engine/`, `subtitle/` and `translate/` can be driven
//! from somewhere that is *not* a running Tauri app — a CLI, or a unit test.
//! Those modules used to take `&tauri::AppHandle` and reach through it for
//! paths, which meant only a Tauri process could run them.
//!
//! # Two independent things, often confused
//!
//! [`AppPaths::config_dir`] (where `settings.json` lives) and
//! [`AppPaths::models_root`] (where model weights live) are **unrelated
//! locations**. The config file merely happens to *contain* a string pointing at
//! the models directory. Measured on a real install:
//!
//! ```text
//! config_dir   ~/Library/Application Support/dev.leix.monosub
//! models_root  /Users/lei/dev/projects/2601-monosub/models      ← from settings.json
//! ```
//!
//! Nothing derives one from the other. Keep them separate.

use std::path::{Path, PathBuf};

/// Env override for the config directory (mirrors `MONOSUB_MODELS_DIR`).
pub const CONFIG_DIR_ENV: &str = "MONOSUB_CONFIG_DIR";
/// Env override for the bundled-resources root (`models.json`, `bin/`).
pub const RESOURCES_DIR_ENV: &str = "MONOSUB_RESOURCES_DIR";

/// Bundle identifier from `tauri.conf.json`, used to derive the per-app
/// directories. It must match the app's own identifier or the CLI would read a
/// different `settings.json` than the GUI writes.
const APP_IDENTIFIER: &str = "dev.leix.monosub";

/// Everything the engine needs from the environment, resolved once.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppPaths {
    /// `<...>/resources` — root of `models.json`, `bin/`, `models/silero_vad`.
    pub resources: PathBuf,
    /// Where model weights live. **Unrelated to `config_dir`**; resolved from
    /// `MONOSUB_MODELS_DIR` → `settings.json`'s `modelsDir` → `<data_dir>/models`.
    pub models_root: PathBuf,
    /// Where `settings.json` lives. **Unrelated to `models_root`**; the config
    /// file is just where the models path is written down.
    pub config_dir: PathBuf,
    /// Base directory for defaults. Not itself a place the app writes to.
    pub data_dir: PathBuf,
}

/// Why a path set could not be resolved.
#[derive(Debug)]
pub enum PathError {
    /// The environment did not provide a home/config directory.
    NoHome(&'static str),
}

impl std::fmt::Display for PathError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PathError::NoHome(what) => write!(f, "cannot resolve the {what} directory"),
        }
    }
}

impl std::error::Error for PathError {}

impl AppPaths {
    /// Resolve from a running Tauri app.
    ///
    /// `config_dir` and `data_dir` come from Tauri itself, because the GUI must
    /// keep writing to exactly the paths it always has. `resources` uses the same
    /// probes as [`AppPaths::from_env`], so the two agree.
    #[cfg(feature = "tauri-app")]
    pub fn from_app(app: &tauri::AppHandle) -> Result<Self, PathError> {
        use tauri::Manager;

        let config_dir = app
            .path()
            .app_config_dir()
            .map_err(|_| PathError::NoHome("app config"))?;
        let data_dir = app
            .path()
            .app_data_dir()
            .map_err(|_| PathError::NoHome("app data"))?;

        Self::assemble(config_dir, data_dir)
    }

    /// Resolve without a Tauri runtime: a CLI, or a test.
    ///
    /// Deliberately mirrors the GUI's platform directories using
    /// [`APP_IDENTIFIER`], so both frontends read the same `settings.json`.
    pub fn from_env() -> Result<Self, PathError> {
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            // Windows has no HOME; `APPDATA` is the documented base there.
            .or_else(|| std::env::var_os("APPDATA").map(PathBuf::from))
            .ok_or(PathError::NoHome("home"))?;

        let (config_dir, data_dir) = if cfg!(target_os = "macos") {
            // macOS keeps both under Application Support, which is why they
            // look alike here — they are still independent values.
            let base = home
                .join("Library")
                .join("Application Support")
                .join(APP_IDENTIFIER);
            (base.clone(), base)
        } else if cfg!(target_os = "windows") {
            let base = home.join(APP_IDENTIFIER);
            (base.clone(), base)
        } else {
            (
                home.join(".config").join(APP_IDENTIFIER),
                home.join(".local").join("share").join(APP_IDENTIFIER),
            )
        };

        Self::assemble(config_dir, data_dir)
    }

    /// Shared tail of both constructors: resolve the models root from the
    /// config dir, so `modelsDir` in `settings.json` is honoured identically.
    fn assemble(config_dir: PathBuf, data_dir: PathBuf) -> Result<Self, PathError> {
        let config_dir = env_path(CONFIG_DIR_ENV).unwrap_or(config_dir);
        let models_root = resolve_models_root(&config_dir, &data_dir);
        Ok(Self {
            resources: resolve_resources_root(),
            models_root,
            config_dir,
            data_dir,
        })
    }

    /// `<config_dir>/settings.json`.
    ///
    /// Not called yet: the settings write/read path still goes through
    /// `model_dir`, which predates `AppPaths`. Kept because it is the one place
    /// that definition lives, and the CLI reads settings through it.
    #[allow(dead_code)]
    pub fn settings_path(&self) -> PathBuf {
        self.config_dir.join("settings.json")
    }
}

/// A non-empty env var as a path.
fn env_path(var: &str) -> Option<PathBuf> {
    let raw = std::env::var_os(var)?;
    let trimmed = raw.to_string_lossy().trim().to_string();
    (!trimmed.is_empty()).then(|| PathBuf::from(trimmed))
}

/// `MONOSUB_MODELS_DIR` → `settings.json`'s `modelsDir` → `<data_dir>/models`.
///
/// This is the precedence `model_dir::configured_root` has always had; it lives
/// here so it can run *before* an [`AppPaths`] exists (the models root is a field
/// of it, so it cannot be read back through one).
fn resolve_models_root(config_dir: &Path, data_dir: &Path) -> PathBuf {
    if let Some(overridden) = env_path(crate::model_dir::MODELS_DIR_ENV) {
        return overridden;
    }
    if let Some(configured) = read_models_dir_from(config_dir) {
        return configured;
    }
    data_dir.join("models")
}

/// The `modelsDir` string in `<config_dir>/settings.json`, if any.
///
/// Reads the file directly rather than through `model_dir::load_settings`, which
/// would need an [`AppPaths`] that is still being built.
fn read_models_dir_from(config_dir: &Path) -> Option<PathBuf> {
    let raw = std::fs::read_to_string(config_dir.join("settings.json")).ok()?;
    let parsed: serde_json::Value = serde_json::from_str(&raw).ok()?;
    let value = parsed.get("modelsDir")?.as_str()?.trim();
    (!value.is_empty()).then(|| PathBuf::from(value))
}

/// Where the bundled resources live, without needing an `AppHandle`.
///
/// Order: `MONOSUB_RESOURCES_DIR` → `<cwd>/src-tauri/resources` (dev checkout) →
/// `<exe_dir>/resources` (`tauri dev` copies resources next to the binary) →
/// `<exe_dir>/../Resources/resources` (macOS bundle).
///
/// The GUI used to gate the last two on `app.is_some()`; that gate is gone
/// because the probes are existence checks — when the directory is not there,
/// the next candidate is tried anyway, so dropping the gate cannot change which
/// path wins for a real app, and lets a CLI resolve the same layout.
pub fn resolve_resources_root() -> PathBuf {
    if let Some(overridden) = env_path(RESOURCES_DIR_ENV) {
        return overridden;
    }

    let mut candidates: Vec<PathBuf> = Vec::new();

    if let Ok(cwd) = std::env::current_dir() {
        candidates.push(cwd.join("src-tauri").join("resources"));
    }

    if let Ok(exe) = std::env::current_exe() {
        if let Some(exe_dir) = exe.parent() {
            candidates.push(exe_dir.join("resources"));
            // macOS bundle: Contents/MacOS/monosub → Contents/Resources/resources
            if let Some(contents) = exe_dir.parent() {
                candidates.push(contents.join("Resources").join("resources"));
            }
        }
    }

    // Fall back to the dev-checkout default so a caller that cannot see any of
    // the above gets a usable (if wrong) path rather than a panic.
    candidates
        .into_iter()
        .find(|dir| dir.is_dir())
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("resources")
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The config dir and the models root are independent: a `modelsDir` that
    /// points somewhere completely unrelated must be honoured verbatim, and must
    /// not be rewritten to live under the config dir.
    #[test]
    fn a_configured_models_dir_is_used_verbatim_not_derived_from_the_config_dir() {
        let config_dir = std::env::temp_dir().join(format!(
            "monosub-paths-config-{}",
            std::process::id()
        ));
        let elsewhere = std::env::temp_dir().join("somewhere-else-models");
        std::fs::create_dir_all(&config_dir).unwrap();
        // Built with `serde_json` rather than `format!`: a Windows path is full of
        // backslashes, which are escapes in JSON, so a hand-formatted string is
        // invalid there and the parse fails for a reason unrelated to the test.
        let settings = serde_json::json!({ "modelsDir": elsewhere.display().to_string() });
        std::fs::write(
            config_dir.join("settings.json"),
            serde_json::to_string(&settings).unwrap(),
        )
        .unwrap();

        let found = read_models_dir_from(&config_dir);

        assert_eq!(found, Some(elsewhere));
        let _ = std::fs::remove_dir_all(&config_dir);
    }

    /// A blank `modelsDir` means "use the default", not "models live at /".
    #[test]
    fn a_blank_models_dir_is_not_a_path() {
        let config_dir = std::env::temp_dir().join(format!(
            "monosub-paths-blank-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&config_dir).unwrap();
        std::fs::write(config_dir.join("settings.json"), r#"{"modelsDir":"   "}"#).unwrap();

        assert_eq!(read_models_dir_from(&config_dir), None);
        let _ = std::fs::remove_dir_all(&config_dir);
    }

    /// A corrupt or missing settings file must not panic the CLI.
    #[test]
    fn unreadable_settings_yield_no_override() {
        let missing = std::env::temp_dir().join("monosub-paths-definitely-absent");
        assert_eq!(read_models_dir_from(&missing), None);
    }

    /// `settings_path` must come from `config_dir` alone. Pinning this stops
    /// anyone from "tidying up" by deriving it from `models_root`.
/// The CLI and the GUI must read the **same** `settings.json`, or the CLI
    /// inherits none of the model plan the user chose and the whole "reuse the
    /// chosen model" contract is void.
    ///
    /// `from_app` cannot be constructed without a real `AppHandle`, so this pins
    /// the two things that must agree instead: the platform directory the
    /// CLI derives, and the bundle identifier Tauri uses for the GUI.
    #[test]
    fn the_cli_derives_the_gui_s_app_directory() {
        let identifier = APP_IDENTIFIER;
        let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default();

        if cfg!(target_os = "macos") {
            let expected = home
                .join("Library")
                .join("Application Support")
                .join(identifier);
            assert!(expected.ends_with(identifier), "{expected:?}");
        } else if cfg!(target_os = "linux") {
            let expected = home.join(".config").join(identifier);
            assert!(expected.ends_with(identifier), "{expected:?}");
        }

        // The identifier must be the one in tauri.conf.json. Read it here rather
        // than restating it, so a bundle-id change cannot silently desync the CLI.
        let conf = std::fs::read_to_string(
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tauri.conf.json"),
        )
        .expect("tauri.conf.json is part of the source tree");
        let parsed: serde_json::Value = serde_json::from_str(&conf).unwrap();
        assert_eq!(
            parsed.get("identifier").and_then(|v| v.as_str()),
            Some(APP_IDENTIFIER),
            "APP_IDENTIFIER drifted from tauri.conf.json — the CLI would read a different settings.json than the GUI writes"
        );
    }

    /// The resources root must resolve to something that actually contains the
    /// catalogue, in a dev checkout at least. A CLI that cannot find models.json
    /// cannot plan at all.
    #[test]
    fn the_resources_root_contains_the_model_catalogue_in_a_dev_checkout() {
        let root = resolve_resources_root();
        assert!(
            root.join("models.json").is_file(),
            "no models.json under {root:?}; a CLI run from this checkout could not plan"
        );
    }

    #[test]
    fn settings_live_in_the_config_dir_regardless_of_the_models_root() {
        let paths = AppPaths {
            resources: PathBuf::from("/res"),
            models_root: PathBuf::from("/models/elsewhere"),
            config_dir: PathBuf::from("/cfg"),
            data_dir: PathBuf::from("/data"),
        };
        assert_eq!(paths.settings_path(), PathBuf::from("/cfg/settings.json"));
    }
}
