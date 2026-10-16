//! `models.json` catalogue: model metadata, install checks and language mapping.
//!
//! Task: P1-5. See docs/tech-spec/refactor-audiocpp.md §5.2 / §5.6.
//!
//! Pinned baseline — only the exports below may change; keep them stable so
//! `engine/mod.rs` (P1-9) and `commands/models.rs` (P1-14) can be written
//! against this signature.

use std::path::{Path, PathBuf};

use crate::engine::EngineError;
use serde::Deserialize;

/// One downloadable file of a model.
#[derive(Debug, Clone)]
pub struct ModelFile {
    pub path: String,
    /// Primary URL followed by fallback mirrors, tried in order.
    pub urls: Vec<String>,
    pub size: u64,
    pub sha256: Option<String>,
}

/// Granularity of timestamps a model can produce.
///
/// Ordered from coarsest to finest so `>=` comparisons read naturally; the
/// solver (design §3.2) uses this to answer "does this cover a word-level
/// requirement?".
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TimestampGrade {
    None,
    Turns,
    Segment,
    Word,
}

/// Whether a model emits speaker information, and whether it also emits text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiarizationGrade {
    None,
    Turns,
    TurnsWithText,
}

impl TimestampGrade {
    /// Stable lowercase name, matching the `models.json` spelling.
    pub fn as_str(self) -> &'static str {
        match self {
            TimestampGrade::None => "none",
            TimestampGrade::Turns => "turns",
            TimestampGrade::Segment => "segment",
            TimestampGrade::Word => "word",
        }
    }
}

impl DiarizationGrade {
    /// Stable lowercase name, matching the `models.json` spelling.
    pub fn as_str(self) -> &'static str {
        match self {
            DiarizationGrade::None => "none",
            DiarizationGrade::Turns => "turns",
            DiarizationGrade::TurnsWithText => "turns_with_text",
        }
    }
}

/// What a model actually provides (design §3.2). Capabilities live in
/// `models.json`, never in Rust, so adding a model is a JSON change.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Capabilities {
    pub timestamps: TimestampGrade,
    pub diarization: DiarizationGrade,
    /// False for diar-only models (they emit turns but no transcript).
    pub produces_text: bool,
}

impl Default for Capabilities {
    /// The v1 back-compat fallback: a plain segment-less ASR model.
    fn default() -> Self {
        Self {
            timestamps: TimestampGrade::Segment,
            diarization: DiarizationGrade::None,
            produces_text: true,
        }
    }
}

/// Role a model can fill in a plan (`engine/plan.rs` step kinds).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    Asr,
    Vad,
    Align,
    Diar,
    Passthrough,
}

impl Role {
    /// Stable lowercase name, matching the `models.json` spelling.
    pub fn as_str(self) -> &'static str {
        match self {
            Role::Asr => "asr",
            Role::Vad => "vad",
            Role::Align => "align",
            Role::Diar => "diar",
            Role::Passthrough => "passthrough",
        }
    }
}

/// Speed class, used only for same-capability ranking (design §5.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Speed {
    Fast,
    Balanced,
    Accurate,
}

/// Upstream provenance, used for the "Tongyi first" tie-break (design §5.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Source {
    Tongyi,
    Nvidia,
    Openmoss,
    Qwen,
    Other,
}

impl Source {
    /// Stable lowercase name, matching the `models.json` spelling.
    pub fn as_str(self) -> &'static str {
        match self {
            Source::Tongyi => "tongyi",
            Source::Nvidia => "nvidia",
            Source::Openmoss => "openmoss",
            Source::Qwen => "qwen",
            Source::Other => "other",
        }
    }
}

/// UI grouping / default visibility (design §4.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Tier {
    Default,
    Recommended,
    Optional,
    Experimental,
}

impl Tier {
    /// Stable lowercase name, matching the `models.json` spelling.
    pub fn as_str(self) -> &'static str {
        match self {
            Tier::Default => "default",
            Tier::Recommended => "recommended",
            Tier::Optional => "optional",
            Tier::Experimental => "experimental",
        }
    }
}

/// One licence attached to a model. `licenses[]` supersedes the single
/// `license` string, which is retained for back-compat.
#[derive(Debug, Clone)]
pub struct License {
    pub name: String,
    pub url: Option<String>,
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
    /// v2 fields. `None` means the entry predates v2 (see [`ModelSpec::capabilities`]).
    pub capabilities_raw: Option<Capabilities>,
    pub roles: Vec<Role>,
    pub speed: Option<Speed>,
    pub source: Option<Source>,
    pub tier: Option<Tier>,
    pub licenses: Vec<License>,
}

impl ModelSpec {
    /// Effective capabilities, applying the v1 fallback when the entry did not
    /// declare any (design §4.2).
    pub fn capabilities(&self) -> Capabilities {
        self.capabilities_raw.unwrap_or_default()
    }

    /// True when the model can fill `role`; a v1 entry with no `roles` defaults
    /// to `["asr"]`.
    pub fn has_role(&self, role: Role) -> bool {
        let roles = self.effective_roles();
        roles.contains(&role)
    }

    fn effective_roles(&self) -> Vec<Role> {
        if self.roles.is_empty() {
            vec![Role::Asr]
        } else {
            self.roles.clone()
        }
    }
}

/// Catalogue plus install state.
#[derive(Debug, Clone)]
pub struct ModelStatus {
    pub spec: ModelSpec,
    /// True when every file exists with a size matching the manifest.
    pub installed: bool,
    pub size_on_disk: u64,
}

// --- `models.json` wire format ---------------------------------------------
// The public types above derive nothing, so the on-disk shape is described by
// these private structs. `files[].url` (primary) followed by the optional
// `files[].mirrors` become `ModelFile::urls` in that order.

#[derive(Deserialize)]
struct CatalogWire {
    models: Vec<ModelWire>,
}

/// `capabilities` as written in JSON; converted into the public `Capabilities`.
#[derive(Deserialize)]
struct CapabilitiesWire {
    timestamps: TimestampGrade,
    diarization: DiarizationGrade,
    #[serde(default = "default_true")]
    produces_text: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Deserialize)]
struct LicenseWire {
    name: String,
    url: Option<String>,
}

#[derive(Deserialize)]
struct ModelWire {
    id: String,
    family: String,
    display_name: String,
    target_directory: String,
    model_file: String,
    files: Vec<FileWire>,
    languages: Vec<String>,
    license: String,
    attribution: Option<String>,
    // v2 fields. `serde(default)` keeps a v1 catalogue loadable.
    #[serde(default)]
    capabilities: Option<CapabilitiesWire>,
    #[serde(default)]
    roles: Vec<Role>,
    #[serde(default)]
    speed: Option<Speed>,
    #[serde(default)]
    source: Option<Source>,
    #[serde(default)]
    tier: Option<Tier>,
    #[serde(default)]
    licenses: Vec<LicenseWire>,
}

#[derive(Deserialize)]
struct FileWire {
    path: String,
    url: String,
    mirrors: Option<Vec<String>>,
    size: u64,
    sha256: Option<String>,
}

/// Parse `models.json` from an explicit path.
pub fn load_catalog(path: &Path) -> Result<Vec<ModelSpec>, EngineError> {
    let raw = std::fs::read_to_string(path)
        .map_err(|e| EngineError::Io(format!("failed to read {}: {}", path.display(), e)))?;
    let parsed: CatalogWire = serde_json::from_str(&raw)
        .map_err(|e| EngineError::Protocol(format!("invalid {}: {}", path.display(), e)))?;

    let mut specs: Vec<ModelSpec> = Vec::with_capacity(parsed.models.len());
    for model in parsed.models {
        let mut files = Vec::with_capacity(model.files.len());
        for file in model.files {
            let mut urls = Vec::with_capacity(1 + file.mirrors.as_ref().map_or(0, Vec::len));
            urls.push(file.url);
            urls.extend(file.mirrors.unwrap_or_default());
            files.push(ModelFile {
                path: file.path,
                urls,
                size: file.size,
                sha256: file.sha256,
            });
        }

        if specs.iter().any(|existing| existing.id == model.id) {
            return Err(EngineError::Protocol(format!(
                "duplicate model id in {}: {}",
                path.display(),
                model.id
            )));
        }

        specs.push(ModelSpec {
            id: model.id,
            family: model.family,
            display_name: model.display_name,
            target_directory: model.target_directory,
            model_file: model.model_file,
            files,
            languages: model.languages,
            license: model.license,
            attribution: model.attribution,
            capabilities_raw: model.capabilities.map(|caps| Capabilities {
                timestamps: caps.timestamps,
                diarization: caps.diarization,
                produces_text: caps.produces_text,
            }),
            roles: model.roles,
            speed: model.speed,
            source: model.source,
            tier: model.tier,
            licenses: model
                .licenses
                .into_iter()
                .map(|license| License {
                    name: license.name,
                    url: license.url,
                })
                .collect(),
        });
    }

    Ok(specs)
}

/// Load the catalogue bundled at `src-tauri/resources/models.json`
/// (dev: repo path; prod: `Resources/resources/models.json`).
pub fn catalog(app: Option<&tauri::AppHandle>) -> Result<Vec<ModelSpec>, EngineError> {
    let path = crate::engine::cli::resolve_catalog_path(app)?;
    load_catalog(&path)
}

/// Absolute model file path for `id`, or `None` when unknown.
pub fn model_path(
    app: &tauri::AppHandle,
    id: &str,
) -> Result<Option<PathBuf>, EngineError> {
    let Some(spec) = lookup(app, id)? else {
        return Ok(None);
    };
    Ok(Some(
        models_root(app)?.join(&spec.target_directory).join(&spec.model_file),
    ))
}

/// `<app_data_dir>/models/<target_directory>`.
pub fn model_dir(app: &tauri::AppHandle, id: &str) -> Result<PathBuf, EngineError> {
    let spec = find(app, id)?;
    Ok(models_root(app)?.join(&spec.target_directory))
}

/// True when every manifest file exists and its size matches (never hashes).
pub fn is_installed(app: &tauri::AppHandle, id: &str) -> bool {
    let Ok(Some(spec)) = lookup(app, id) else {
        return false;
    };
    let Ok(root) = models_root(app) else {
        return false;
    };
    let dir = root.join(&spec.target_directory);
    spec.files
        .iter()
        .all(|file| file_size(&dir.join(&file.path)) == Some(file.size))
}

/// List every model with its install state; used by `list_models`.
pub fn list(status: Option<&tauri::AppHandle>) -> Result<Vec<ModelStatus>, EngineError> {
    let specs = catalog(status)?;
    let root = status.and_then(|app| models_root(app).ok());

    Ok(specs
        .into_iter()
        .map(|spec| {
            let dir = root.as_ref().map(|root| root.join(&spec.target_directory));
            let mut installed = root.is_some();
            let mut size_on_disk = 0u64;
            for file in &spec.files {
                match dir.as_ref().and_then(|dir| file_size(&dir.join(&file.path))) {
                    Some(size) => {
                        size_on_disk += size;
                        installed &= size == file.size;
                    }
                    None => installed = false,
                }
            }
            ModelStatus {
                spec,
                installed,
                size_on_disk,
            }
        })
        .collect())
}

/// `models.json` `default` entry id.
pub fn default_model_id() -> &'static str {
    "sensevoice_small_q8"
}

/// Tags that `sense_asr` maps onto a real language token (PoC §4.2,
/// `language_query_token()`). The CLI *accepts* 23 tags but lowers every other
/// one to `auto` internally, so the correct tag has to be chosen here.
const SENSE_ASR_TOKEN_LANGUAGES: &[&str] = &["auto", "zh", "en", "yue", "ja", "ko", "nospeech"];

/// Map the app language (`zh`, `zh-tw`, `en`, …) onto a CLI `language` tag.
///
/// Per PoC §4.2: the CLI accepts 23 tags but only `{auto, zh, en, yue, ja, ko,
/// nospeech}` map onto a real token, so unsupported values must be lowered to
/// `auto` here rather than relying on the CLI.
pub fn map_language(model: &ModelSpec, app_lang: &str) -> String {
    let tag = normalize_language_tag(app_lang);

    // `sense_asr` is the only family whose real tokens are known to be a strict
    // subset of the tags it accepts. Other families (spec §5.6, `qwen3_asr`) do
    // map every tag they declare, so their `languages` list is authoritative.
    let forwarded = if model.family == "sense_asr" {
        SENSE_ASR_TOKEN_LANGUAGES.contains(&tag.as_str())
    } else {
        model.languages.iter().any(|declared| declared == &tag)
    };

    if forwarded {
        tag
    } else {
        "auto".to_string()
    }
}

/// `zh-tw` shares the `zh` token: neither the previous FunASR engine nor
/// audio.cpp converts to traditional (PoC P0-8), so there is nothing to do
/// beyond normalising the regional tag.
fn normalize_language_tag(app_lang: &str) -> String {
    match app_lang.trim().to_lowercase().as_str() {
        "zh-tw" => "zh".to_string(),
        other => other.to_string(),
    }
}

/// `<app_data_dir>/models`.
/// `<app_data_dir>/models`, or the user's configured directory.
fn models_root(app: &tauri::AppHandle) -> Result<PathBuf, EngineError> {
    // Single source of truth: `model_dir` owns the precedence (env → persisted
    // setting → default), so the catalogue and the downloader can never
    // disagree about where models live.
    crate::model_dir::configured_root(app).map_err(EngineError::Io)
}

fn lookup(app: &tauri::AppHandle, id: &str) -> Result<Option<ModelSpec>, EngineError> {
    Ok(catalog(Some(app))?.into_iter().find(|spec| spec.id == id))
}

fn find(app: &tauri::AppHandle, id: &str) -> Result<ModelSpec, EngineError> {
    lookup(app, id)?.ok_or_else(|| EngineError::Protocol(format!("unknown model id: {}", id)))
}

/// Size of `path`, or `None` when it is missing or not a regular file.
fn file_size(path: &Path) -> Option<u64> {
    std::fs::metadata(path)
        .ok()
        .filter(|meta| meta.is_file())
        .map(|meta| meta.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    const HF_URL: &str = "https://huggingface.co/FunAudioLLM/SenseVoiceSmall-GGUF-audiocpp/resolve/5c3fcfe748a8714216bc135476d5863084fddb72/sensevoice-small-q8-audiocpp-v1.gguf";
    const HF_MIRROR_URL: &str = "https://hf-mirror.com/FunAudioLLM/SenseVoiceSmall-GGUF-audiocpp/resolve/5c3fcfe748a8714216bc135476d5863084fddb72/sensevoice-small-q8-audiocpp-v1.gguf";

    fn committed_catalog() -> Vec<ModelSpec> {
        let path = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/resources/models.json"));
        load_catalog(path).expect("committed models.json parses")
    }

    fn sensevoice_spec() -> ModelSpec {
        committed_catalog()
            .into_iter()
            .find(|spec| spec.id == "sensevoice_small_q8")
            .expect("models.json declares sensevoice_small_q8")
    }

    #[test]
    fn load_catalog_parses_committed_sensevoice_entry() {
        let spec = sensevoice_spec();

        assert_eq!(spec.id, default_model_id());
        assert_eq!(spec.family, "sense_asr");
        assert_eq!(spec.display_name, "SenseVoice Small (Q8)");
        assert_eq!(spec.target_directory, "SenseVoice-Small-GGUF");
        assert_eq!(spec.model_file, "sensevoice-small-q8-audiocpp-v1.gguf");
        assert_eq!(spec.license, "FunASR Model License v1.1");
        assert_eq!(
            spec.attribution.as_deref(),
            Some("SenseVoice-Small (FunAudioLLM/SenseVoiceSmall)")
        );
        assert_eq!(
            spec.languages,
            vec![
                "auto", "zh", "en", "yue", "ja", "ko", "pt", "ru", "es", "it", "fr", "de", "nl",
                "pl", "tr", "ar", "hi", "vi", "th", "id", "ms", "fa",
            ]
        );

        assert_eq!(spec.files.len(), 1);
        let file = &spec.files[0];
        assert_eq!(file.path, "sensevoice-small-q8-audiocpp-v1.gguf");
        assert_eq!(file.size, 254_211_200);
        assert_eq!(
            file.sha256.as_deref(),
            Some("4dedf169f625437fb336f2959674f399819729a765e184128c0e25a6e16ff0ec")
        );
        // Primary URL first, mirrors in manifest order after it.
        assert_eq!(
            file.urls,
            vec![HF_URL.to_string(), HF_MIRROR_URL.to_string()]
        );
    }

    /// v2 fields parse on a real committed entry, including the derived helpers.
    #[test]
    fn load_catalog_parses_v2_capability_fields() {
        let specs = committed_catalog();
        let moss = specs
            .iter()
            .find(|spec| spec.id == "moss_transcribe_diarize_q8_0")
            .expect("models.json declares moss_transcribe_diarize_q8_0");

        let caps = moss.capabilities();
        assert_eq!(caps.timestamps, TimestampGrade::Segment);
        assert_eq!(caps.diarization, DiarizationGrade::TurnsWithText);
        assert!(caps.produces_text);
        assert!(moss.has_role(Role::Passthrough));
        assert!(moss.has_role(Role::Asr));
        assert!(!moss.has_role(Role::Diar));
        assert_eq!(moss.speed, Some(Speed::Balanced));
        assert_eq!(moss.source, Some(Source::Openmoss));
        assert_eq!(moss.tier, Some(Tier::Recommended));
        assert_eq!(moss.licenses.len(), 1);
        assert_eq!(moss.licenses[0].name, "Apache-2.0");

        // The existing entry must keep its exact download data (regression guard).
        let sense = sensevoice_spec();
        assert_eq!(sense.capabilities().timestamps, TimestampGrade::None);
        assert_eq!(sense.capabilities().diarization, DiarizationGrade::None);
        assert_eq!(sense.source, Some(Source::Tongyi));
        assert_eq!(sense.tier, Some(Tier::Default));
    }

    /// A diar-only model must not claim text, and the aligner must not either.
    #[test]
    fn load_catalog_distinguishes_diar_only_models() {
        let specs = committed_catalog();
        let sortformer = specs
            .iter()
            .find(|spec| spec.id == "sortformer_diar_4spk_q8_0")
            .expect("sortformer entry present");
        assert_eq!(sortformer.capabilities().diarization, DiarizationGrade::Turns);
        assert!(!sortformer.capabilities().produces_text);
        assert!(sortformer.has_role(Role::Diar));
    }

    /// The catalogue must not offer a model MonoSub cannot drive: the `align`
    /// task aligns an EXISTING transcript to audio, and this app only ever
    /// produces transcripts. Keeping the entry would let the solver plan a step
    /// that never runs.
    #[test]
    fn catalogue_has_no_align_only_model() {
        let specs = committed_catalog();
        assert!(
            specs.iter().all(|spec| !spec.has_role(Role::Align)),
            "no catalogue entry may require the unsupported `align` task"
        );
        assert!(
            specs.iter().all(|spec| spec.family != "qwen3_forced_aligner"),
            "the forced-aligner entry was removed deliberately"
        );
    }

    /// `fun_asr_nano` does not support ko/yue; the catalogue must reflect that.
    #[test]
    fn fun_asr_nano_languages_exclude_ko_and_yue() {
        let specs = committed_catalog();
        let nano = specs
            .iter()
            .find(|spec| spec.id == "fun_asr_nano_2512_q8_0")
            .expect("fun_asr_nano entry present");
        assert!(!nano.languages.iter().any(|lang| lang == "ko"));
        assert!(!nano.languages.iter().any(|lang| lang == "yue"));
        assert!(nano.languages.iter().any(|lang| lang == "zh"));
    }

    /// A v1-shaped entry (no `capabilities`, no `roles`) still loads and gets the
    /// documented fallback: a plain segment-less ASR model.
    #[test]
    fn v1_entry_without_v2_fields_falls_back() {
        let temp = std::env::temp_dir().join(format!(
            "monosub-catalog-v1-{}.json",
            std::process::id()
        ));
        std::fs::write(
            &temp,
            r#"{
              "version": 1,
              "models": [
                {
                  "id": "legacy",
                  "family": "sense_asr",
                  "display_name": "Legacy",
                  "target_directory": "legacy",
                  "model_file": "legacy.gguf",
                  "files": [],
                  "languages": ["auto"],
                  "license": "MIT"
                }
              ]
            }"#,
        )
        .unwrap();

        let specs = load_catalog(&temp).expect("v1 catalogue loads");
        std::fs::remove_file(&temp).unwrap();

        let legacy = &specs[0];
        assert_eq!(legacy.capabilities(), Capabilities::default());
        assert!(legacy.has_role(Role::Asr), "missing roles defaults to asr");
        assert!(legacy.licenses.is_empty());
        assert_eq!(legacy.speed, None);
    }

    #[test]
    fn load_catalog_rejects_unreadable_or_malformed_files() {
        let missing = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/resources/does-not-exist.json"));
        assert!(matches!(load_catalog(missing), Err(EngineError::Io(_))));

        let temp = std::env::temp_dir().join(format!("monosub-catalog-{}.json", std::process::id()));
        std::fs::write(&temp, "{ not json").unwrap();
        assert!(matches!(load_catalog(&temp), Err(EngineError::Protocol(_))));
        std::fs::remove_file(&temp).unwrap();
    }

    /// Every `OutputLanguage` in `src/store/useStore.ts` plus an unknown tag.
    #[test]
    fn map_language_covers_every_app_language() {
        let model = sensevoice_spec();
        let table: &[(&str, &str)] = &[
            ("auto", "auto"),
            ("en", "en"),
            ("zh", "zh"),
            ("es", "auto"),
            ("ja", "ja"),
            ("de", "auto"),
            ("fr", "auto"),
            ("pt", "auto"),
            ("zh-tw", "zh"),
            ("ko", "ko"),
            ("zh-TW", "zh"),
            ("xx", "auto"),
            ("", "auto"),
        ];

        for (app_lang, expected) in table {
            assert_eq!(
                map_language(&model, app_lang),
                *expected,
                "map_language({:?})",
                app_lang
            );
        }
    }

    /// A family whose `languages` list is authoritative (spec §5.6 `qwen3_asr`).
    #[test]
    fn map_language_uses_declared_languages_for_other_families() {
        let model = ModelSpec {
            id: "qwen3_asr_0_6b_q8_0".to_string(),
            family: "qwen3_asr".to_string(),
            display_name: "Qwen3 ASR".to_string(),
            target_directory: "qwen3_asr".to_string(),
            model_file: "qwen3-asr.gguf".to_string(),
            files: Vec::new(),
            languages: vec!["zh".to_string(), "en".to_string(), "es".to_string()],
            license: "Apache-2.0".to_string(),
            attribution: None,
            capabilities_raw: None,
            roles: Vec::new(),
            speed: None,
            source: None,
            tier: None,
            licenses: Vec::new(),
        };

        assert_eq!(map_language(&model, "es"), "es");
        assert_eq!(map_language(&model, "zh-tw"), "zh");
        assert_eq!(map_language(&model, "de"), "auto");
    }
}
