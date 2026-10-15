//! `models.json` catalogue: model metadata, install checks and language mapping.
//!
//! Task: P1-5. See docs/tech-spec/refactor-audiocpp.md §5.2 / §5.6.
//!
//! Pinned baseline — only the exports below may change; keep them stable so
//! `engine/mod.rs` (P1-9) and `commands/models.rs` (P1-14) can be written
//! against this signature.

use std::path::{Path, PathBuf};

/// One downloadable file of a model.
#[derive(Debug, Clone)]
pub struct ModelFile {
    pub path: String,
    /// Primary URL followed by fallback mirrors, tried in order.
    pub urls: Vec<String>,
    pub size: u64,
    pub sha256: Option<String>,
}

/// One model entry from `models.json`.
#[derive(Debug, Clone)]
pub struct ModelSpec {
    pub id: String,
    /// audio.cpp `family` (e.g. `sense_asr`).
    pub family: String,
    pub display_name: String,
    /// Directory name under `<app_data_dir>/models/`.
    pub target_directory: String,
    /// Relative path of the model file inside `target_directory`.
    pub model_file: String,
    pub files: Vec<ModelFile>,
    /// Language tags accepted by the CLI (23 entries for `sense_asr`).
    pub languages: Vec<String>,
    pub license: String,
    pub attribution: Option<String>,
}

/// Catalogue plus install state.
#[derive(Debug, Clone)]
pub struct ModelStatus {
    pub spec: ModelSpec,
    /// True when every file exists with a size matching the manifest.
    pub installed: bool,
    pub size_on_disk: u64,
}

/// Parse `models.json` from an explicit path.
pub fn load_catalog(path: &Path) -> Result<Vec<ModelSpec>, crate::engine::EngineError> {
    unimplemented!("P1-5")
}

/// Load the catalogue bundled at `src-tauri/resources/models.json`
/// (dev: repo path; prod: `Resources/resources/models.json`).
pub fn catalog(app: Option<&tauri::AppHandle>) -> Result<Vec<ModelSpec>, crate::engine::EngineError> {
    unimplemented!("P1-5")
}

/// Absolute model file path for `id`, or `None` when unknown.
pub fn model_path(app: &tauri::AppHandle, id: &str) -> Result<Option<PathBuf>, crate::engine::EngineError> {
    unimplemented!("P1-5")
}

/// `<app_data_dir>/models/<target_directory>`.
pub fn model_dir(app: &tauri::AppHandle, id: &str) -> Result<PathBuf, crate::engine::EngineError> {
    unimplemented!("P1-5")
}

/// True when every manifest file exists and its size matches (never hashes).
pub fn is_installed(app: &tauri::AppHandle, id: &str) -> bool {
    unimplemented!("P1-5")
}

/// List every model with its install state; used by `list_models`.
pub fn list(status: Option<&tauri::AppHandle>) -> Result<Vec<ModelStatus>, crate::engine::EngineError> {
    unimplemented!("P1-5")
}

/// `models.json` `default` entry id.
pub fn default_model_id() -> &'static str {
    "sensevoice_small_q8"
}

/// Map the app language (`zh`, `zh-tw`, `en`, …) onto a CLI `language` tag.
///
/// Per PoC §4.2: the CLI accepts 23 tags but only `{auto, zh, en, yue, ja, ko,
/// nospeech}` map onto a real token, so unsupported values must be lowered to
/// `auto` here rather than relying on the CLI.
pub fn map_language(_model: &ModelSpec, app_lang: &str) -> String {
    unimplemented!("P1-5")
}
