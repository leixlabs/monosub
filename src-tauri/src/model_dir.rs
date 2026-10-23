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
/// `models_dir` is the user's override; `recognition_plan` and `translation`
/// exist because the CLI is a separate process with no WebView and therefore no
/// `localStorage`. Without them the model the GUI previewed and the model the CLI
/// runs could differ, which is the whole point of the four plan questions.
///
/// ### Why these structs live in `model_dir` and not in a new `settings.rs`
///
/// Settings are already read and written here (`load_settings` / `save_settings`),
/// and `commands::models::set_models_dir` calls them by this path. A second module
/// would either duplicate the read-modify-write rule or make every caller import
/// from a new place, for no gain: the file's only job is to hold a few strings,
/// and **none of them derive a path from another**. `config_dir` (where this file
/// is) and `models_root` (where weights are) stay unrelated — see `paths.rs`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BackendSettings {
    /// User-chosen models directory. `None` means "use the default".
    #[serde(default, rename = "modelsDir", skip_serializing_if = "Option::is_none")]
    pub models_dir: Option<String>,

    /// The plan the GUI solved from the four answers. `None` = the user has not
    /// opened the plan settings yet, and the CLI must fall back to defaults
    /// rather than guess.
    ///
    /// `rename` is per-field rather than a container `rename_all`: `modelsDir`
    /// already had an explicit rename, and the DTOs shipped before this field
    /// existed, so the wire spelling is pinned where it is read.
    #[serde(
        default,
        rename = "recognitionPlan",
        skip_serializing_if = "Option::is_none"
    )]
    pub recognition_plan: Option<RecognitionPlan>,

    /// Translation intent. The CLI's own default is still "off unless
    /// `--translate`"; `enabled` matters to the GUI only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub translation: Option<TranslationSettings>,
}

/// The four plan answers **and the plan they produced**, frozen at Settings time.
///
/// `#[serde(default)]` on the container is deliberate: a cached hint must never
/// make the whole file unreadable, because `BackendSettings` parses
/// all-or-nothing and a dropped `modelsDir` would silently point the app at an
/// empty default directory.
///
/// This is the authority, not a cache: `start_transcription` reads `steps` and
/// runs exactly those models, so a model deleted from disk is reported as
/// `MODEL_NOT_INSTALLED:<id>` rather than silently replaced by a different one
/// the solver would have preferred that day. The user picks once, in Settings;
/// a run does not re-solve.
///
/// `steps` is the whole plan in executable order, including the pure-Rust
/// `turn_attribution` step (which has an empty `model_id`). Storing the steps
/// rather than only `model_ids` is what makes the order and the role of each
/// model recoverable without consulting the catalogue.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct RecognitionPlan {
    /// 语种场景: `mandarin` / `mixed` / `multilingual`.
    pub language_profile: String,
    /// 显存占用: `small` / `medium` / `large`.
    pub memory_budget: String,
    /// 词级时间戳.
    pub require_word_timestamps: bool,
    /// 说话人分离.
    pub diarization: bool,
    /// The language the plan was solved for (`auto` is never a plan input).
    pub language: String,
    /// Model ids in plan order — the download prompts and the install gate read
    /// this, so it stays even though `steps` carries the same information.
    pub model_ids: Vec<String>,
    /// The executable plan, frozen. Empty means "nothing could be solved", which
    /// is the gap plan and must not be run.
    pub steps: Vec<PlanStepRecord>,
}

/// One step of a frozen plan.
///
/// Deliberately its own type rather than reusing `engine::plan::PlanStep`: that
/// one is an in-memory value whose `kind` has no stable spelling, while this is
/// written to `settings.json` and read back by a different process. The `kind`
/// string is the same one [`crate::engine::plan::StepKind::label`] produces
/// (`asr` / `diar` / `turns`), pinned here so a rename in the solver cannot
/// silently reinterpret an existing settings file.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PlanStepRecord {
    /// `asr` / `diar` / `turns` (and `vad` for a manually pinned VAD model).
    pub kind: String,
    /// Catalogue id; empty for the pure-Rust `turns` step.
    pub model_id: String,
    /// audio.cpp `--task`; empty for the pure-Rust step.
    pub task: String,
    /// audio.cpp `--family`; empty for the pure-Rust step.
    pub family: String,
}

/// The Settings → 翻译 panel's intent, persisted so the CLI can report the same
/// choice the GUI shows. `model_id: None` means "the catalogue default".
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct TranslationSettings {
    pub enabled: bool,
    /// `en` / `zh` / `zh-tw` … — never `auto` (that is a source value).
    pub target_language: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_id: Option<String>,
}

/// `<app_config_dir>/settings.json`.
///
/// Resolved from the app config dir alone. `AppPaths::settings_path()` is the
/// same definition for callers that hold an `AppPaths` (the CLI); neither is
/// derived from `models_root`.
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
    match settings_path(app) {
        Ok(path) => read_settings_at(&path),
        Err(_) => BackendSettings::default(),
    }
}

/// Persist the backend settings, creating `<app_config_dir>` when needed.
pub fn save_settings(app: &tauri::AppHandle, settings: &BackendSettings) -> Result<(), String> {
    write_settings_at(&settings_path(app)?, settings)
}

/// Read a settings file by path — the whole of [`load_settings`] without an
/// `AppHandle`, so the behaviour is unit-testable and reusable by the CLI.
pub fn read_settings_at(path: &Path) -> BackendSettings {
    let Ok(raw) = std::fs::read_to_string(path) else {
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

/// Write a settings file by path, creating its parent directory when needed.
pub fn write_settings_at(path: &Path, settings: &BackendSettings) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("failed to create {}: {}", parent.display(), e))?;
    }
    let body = serde_json::to_string_pretty(settings)
        .map_err(|e| format!("failed to serialize settings: {}", e))?;
    std::fs::write(path, body).map_err(|e| format!("failed to write {}: {}", path.display(), e))
}

/// Fold a freshly solved plan into the settings **already on disk**.
///
/// Every writer here is read-modify-write for one reason: `modelsDir`, the plan
/// and the translation intent share one file, so writing a struct built from
/// scratch would delete the other two. The pure halves are split out so a test
/// can prove that without an `AppHandle`.
pub fn merge_recognition_plan(existing: &mut BackendSettings, plan: RecognitionPlan) {
    existing.recognition_plan = Some(plan);
}

/// Fold a translation intent into the settings already on disk.
pub fn merge_translation_settings(
    existing: &mut BackendSettings,
    translation: TranslationSettings,
) {
    existing.translation = Some(translation);
}

/// Persist the plan the GUI just solved, keeping every other field.
pub fn save_recognition_plan(app: &tauri::AppHandle, plan: RecognitionPlan) -> Result<(), String> {
    let path = settings_path(app)?;
    let mut settings = read_settings_at(&path);
    merge_recognition_plan(&mut settings, plan);
    write_settings_at(&path, &settings)
}

/// Persist the translation intent, keeping every other field.
pub fn save_translation_settings(
    app: &tauri::AppHandle,
    translation: TranslationSettings,
) -> Result<(), String> {
    let path = settings_path(app)?;
    let mut settings = read_settings_at(&path);
    merge_translation_settings(&mut settings, translation);
    write_settings_at(&path, &settings)
}

// `configured_root` used to live here. It now exists exactly once, as
// `paths::AppPaths::models_root`, because the precedence (env → persisted
// setting → default) had to run *before* an `AppPaths` could be built — keeping
// a second copy here would let the catalogue and the downloader disagree again,
// which is the one thing this module was written to prevent.

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
            ..BackendSettings::default()
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

    // --- recognitionPlan / translation persistence (R3) -------------------

    fn sample_plan() -> RecognitionPlan {
        RecognitionPlan {
            language_profile: "mixed".to_string(),
            memory_budget: "small".to_string(),
            require_word_timestamps: true,
            diarization: true,
            language: "zh".to_string(),
            model_ids: vec![
                "qwen3_asr_0_6b_q8_0".to_string(),
                "qwen3_forced_aligner_0_6b_q8_0".to_string(),
            ],
            steps: vec![
                PlanStepRecord {
                    kind: "asr".to_string(),
                    model_id: "qwen3_asr_0_6b_q8_0".to_string(),
                    task: "asr".to_string(),
                    family: "qwen3_asr".to_string(),
                },
                PlanStepRecord {
                    kind: "align".to_string(),
                    model_id: "qwen3_forced_aligner_0_6b_q8_0".to_string(),
                    task: "align".to_string(),
                    family: "qwen3_forced_aligner".to_string(),
                },
            ],
        }
    }

    /// An installed app already has `{"modelsDir": "/x"}` on disk. Upgrading
    /// must read it as "no plan chosen yet", not fail and lose the models path.
    #[test]
    fn a_settings_file_with_only_models_dir_still_parses_with_no_plan() {
        let dir = scratch("legacy");
        let path = dir.join(SETTINGS_FILE);
        std::fs::write(&path, r#"{"modelsDir":"/x"}"#).unwrap();

        let settings = read_settings_at(&path);

        assert_eq!(settings.models_dir.as_deref(), Some("/x"));
        assert!(settings.recognition_plan.is_none());
        assert!(settings.translation.is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Whatever is written must read back field for field, including the
    /// camelCase wire names the CLI will look for.
    #[test]
    fn a_recognition_plan_round_trips_through_the_file() {
        let dir = scratch("plan-round-trip");
        let path = dir.join(SETTINGS_FILE);

        let mut settings = BackendSettings::default();
        merge_recognition_plan(&mut settings, sample_plan());
        merge_translation_settings(
            &mut settings,
            TranslationSettings {
                enabled: true,
                target_language: "en".to_string(),
                model_id: Some("hy_mt_1_8b_q4_k_m".to_string()),
            },
        );
        write_settings_at(&path, &settings).unwrap();

        let raw = std::fs::read_to_string(&path).unwrap();
        // The field names are part of the contract with the CLI, so pin them.
        for key in [
            "\"recognitionPlan\"",
            "\"languageProfile\"",
            "\"memoryBudget\"",
            "\"requireWordTimestamps\"",
            "\"diarization\"",
            "\"language\"",
            "\"modelIds\"",
            "\"translation\"",
            "\"enabled\"",
            "\"targetLanguage\"",
            "\"modelId\"",
        ] {
            assert!(raw.contains(key), "settings.json lost {key}:\n{raw}");
        }

        let parsed = read_settings_at(&path);
        assert_eq!(parsed.recognition_plan, Some(sample_plan()));
        assert_eq!(
            parsed.translation,
            Some(TranslationSettings {
                enabled: true,
                target_language: "en".to_string(),
                model_id: Some("hy_mt_1_8b_q4_k_m".to_string()),
            })
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The bug this guards against: a writer that builds a fresh
    /// `BackendSettings` deletes the two fields it was not told about. Every
    /// writer here is read-modify-write, so each one must keep the others.
    #[test]
    fn writing_one_section_keeps_the_others() {
        let dir = scratch("merge");
        let path = dir.join(SETTINGS_FILE);

        let mut settings = BackendSettings {
            models_dir: Some("/models/elsewhere".to_string()),
            ..BackendSettings::default()
        };
        merge_recognition_plan(&mut settings, sample_plan());
        write_settings_at(&path, &settings).unwrap();

        // Translation arrives later (Settings → 翻译) and must not disturb the
        // two fields already on disk.
        let mut reloaded = read_settings_at(&path);
        merge_translation_settings(
            &mut reloaded,
            TranslationSettings {
                enabled: false,
                target_language: "zh-tw".to_string(),
                model_id: None,
            },
        );
        write_settings_at(&path, &reloaded).unwrap();

        let after = read_settings_at(&path);
        assert_eq!(after.models_dir.as_deref(), Some("/models/elsewhere"));
        assert_eq!(after.recognition_plan, Some(sample_plan()));
        assert_eq!(
            after
                .translation
                .as_ref()
                .map(|t| t.target_language.as_str()),
            Some("zh-tw")
        );

        // And the reverse order: a new plan must keep the translation intent.
        let mut reloaded = read_settings_at(&path);
        merge_recognition_plan(
            &mut reloaded,
            RecognitionPlan {
                memory_budget: "large".to_string(),
                ..sample_plan()
            },
        );
        write_settings_at(&path, &reloaded).unwrap();

        let after = read_settings_at(&path);
        assert_eq!(after.models_dir.as_deref(), Some("/models/elsewhere"));
        assert!(
            after.translation.is_some(),
            "the translation intent was lost"
        );
        assert_eq!(
            after.recognition_plan.map(|p| p.memory_budget),
            Some("large".to_string()),
            "the newest plan must win"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A file written by a future build (or by hand) may carry keys this build
    /// does not know. Serde ignores them by default; this pins that, because
    /// `#[serde(deny_unknown_fields)]` here would make the CLI reject the GUI's
    /// own settings after any schema addition.
    #[test]
    fn unknown_keys_do_not_break_the_settings_file() {
        let raw = r#"{
            "modelsDir": "/x",
            "someFutureSection": { "a": 1 },
            "anotherOne": [1, 2, 3]
        }"#;
        let parsed: BackendSettings = serde_json::from_str(raw).expect("unknown keys are ignored");
        assert_eq!(parsed.models_dir.as_deref(), Some("/x"));
        assert!(parsed.recognition_plan.is_none());
    }

    /// A partially-known plan (an older blob, or a hand-edited one) must not
    /// take `modelsDir` down with it: the container is `#[serde(default)]`, so
    /// absent plan fields fall back to defaults instead of failing the file.
    #[test]
    fn a_partial_recognition_plan_does_not_fail_the_whole_file() {
        let raw = r#"{
            "modelsDir": "/x",
            "recognitionPlan": { "languageProfile": "mixed" }
        }"#;
        let parsed: BackendSettings = serde_json::from_str(raw).expect("partial plan still parses");
        assert_eq!(parsed.models_dir.as_deref(), Some("/x"));
        let plan = parsed
            .recognition_plan
            .expect("the plan section is present");
        assert_eq!(plan.language_profile, "mixed");
        assert_eq!(plan.memory_budget, "");
        assert!(!plan.require_word_timestamps);
        assert!(plan.model_ids.is_empty());
    }

    /// `None` sections must be absent from the file, not `null`, so an install
    /// that never opened the plan settings looks unchanged to the CLI.
    #[test]
    fn absent_sections_serialize_away() {
        let body = serde_json::to_string(&BackendSettings::default()).unwrap();
        assert!(!body.contains("recognitionPlan"), "{body}");
        assert!(!body.contains("translation"), "{body}");
        assert!(!body.contains("null"), "{body}");
    }

    /// `resolve_model_plan` degrades a write failure to a warning, which is only
    /// sound if a failure is an `Err` rather than a panic.
    #[test]
    fn an_unwritable_location_is_an_error_not_a_panic() {
        let dir = scratch("unwritable");
        let blocker = dir.join("not-a-directory");
        std::fs::write(&blocker, b"x").unwrap();
        let path = blocker.join(SETTINGS_FILE);

        let error = write_settings_at(&path, &BackendSettings::default())
            .expect_err("a file cannot be a parent directory");
        assert!(error.contains("failed to create"), "{error}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// `modelId: None` means "the catalogue default"; emitting `null` would
    /// make the CLI distinguish two spellings of the same intent.
    #[test]
    fn a_default_translation_model_is_omitted() {
        let body = serde_json::to_string(&TranslationSettings {
            enabled: true,
            target_language: "en".to_string(),
            model_id: None,
        })
        .unwrap();
        assert!(!body.contains("modelId"), "{body}");
    }
}
