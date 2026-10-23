//! `models.json` catalogue: model metadata, install checks and language mapping.
//!
//! Task: P1-5. See docs/tech-spec/refactor-audiocpp.md §5.2 / §5.6.
//!
//! Pinned baseline — only the exports below may change; keep them stable so
//! `engine/mod.rs` (P1-9) and `commands/models.rs` (P1-14) can be written
//! against this signature.

use std::path::{Path, PathBuf};

use crate::engine::EngineError;
use crate::paths::AppPaths;
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
///
/// `Gen` and `Unknown` exist so the catalogue can carry entries this app does
/// not run yet (the audio.cpp build also ships `tts`/`gen`/`s2s`/`svc` loaders).
/// They are **inert**: `plan::solve` only ever looks for the five recognition
/// roles plus `translate`, so such an entry can never enter a plan, and
/// `commands::models` groups it out of the recognition and translation lists.
/// Parsing them is still required — one unrecognised `role` string would fail
/// the whole `models.json` load and take every working model down with it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    Asr,
    Vad,
    Align,
    Diar,
    Passthrough,
    /// Local subtitle translation (translation-design §6). Never part of a
    /// recognition plan: the solver filters translate models out before it
    /// looks for candidates (see `plan::solve`).
    Translate,
    /// Audio/music generation. Not implementable here: this app's pipeline is
    /// transcription → subtitle, with no audio output stage.
    Gen,
    /// Any role this build does not know, kept loadable rather than fatal.
    #[serde(other)]
    Other,
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
            Role::Translate => "translate",
            Role::Gen => "gen",
            Role::Other => "other",
        }
    }
}

/// Which inference engine runs a model.
///
/// Recognition goes through audio.cpp; translation goes through llama.cpp
/// (translation-design §3.2, §11). Kept as a field rather than a second type so
/// the catalogue, the downloader and the install checks stay one code path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Engine {
    /// Absent `engine` in JSON means audio.cpp, so every v2 entry is unchanged.
    #[default]
    Audiocpp,
    Llamacpp,
    /// Python inference stacks (`transformers` / `diffusers` / `pytorch`).
    ///
    /// Same reasoning as [`Role::Other`]: they can never run here — this project
    /// has no Python runtime by design (see AGENTS.md) — but they must parse, so
    /// that a catalogue which describes them stays loadable. `Unknown` absorbs
    /// any future name.
    #[serde(other)]
    Unknown,
}

impl Engine {
    /// Stable lowercase name, matching the `models.json` spelling.
    pub fn as_str(self) -> &'static str {
        match self {
            Engine::Audiocpp => "audiocpp",
            Engine::Llamacpp => "llamacpp",
            Engine::Unknown => "unknown",
        }
    }

    /// True for the two engines this app can actually spawn.
    ///
    /// `transformers`/`diffusers`/`pytorch` entries are catalogue-only: the app
    /// has no Python runtime, so they are listed (so the survey stays readable in
    /// one place) and never offered for download.
    pub fn is_runnable(self) -> bool {
        matches!(self, Engine::Audiocpp | Engine::Llamacpp)
    }
}

/// Speed class, used only for same-capability ranking (design §5.3).
///
/// `Slow` is not cosmetic: the catalogue gained real 7–14B translation models
/// (`alma_13b`, `qwen3_14b`, `mistral_7b_q8_0`) whose whole point is that they
/// are *slower* than the 1.8B default. Without it those entries would fail to
/// deserialize, i.e. one bad model would take the whole catalogue down.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Speed {
    Fast,
    Balanced,
    Accurate,
    Slow,
}

impl Speed {
    pub fn as_str(self) -> &'static str {
        match self {
            Speed::Fast => "fast",
            Speed::Balanced => "balanced",
            Speed::Accurate => "accurate",
            Speed::Slow => "slow",
        }
    }
}

/// Upstream provenance, used for the "Tongyi first" tie-break (design §5.3) and
/// for grouping the model list by vendor.
///
/// This is a closed enum in Rust but an open field on the wire: `#[serde(other)]`
/// maps any vendor the code does not know onto `Other`, so adding a catalogue
/// entry for a new vendor (Cohere, Google, Meta, …) never breaks deserialization.
/// `Other` still means "no bias" in the solver, which is the correct default for
/// a vendor-specific tie-break we did not write a rule for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Source {
    Tongyi,
    Nvidia,
    Openmoss,
    Qwen,
    Cohere,
    Google,
    Meta,
    Mistral,
    Unbabel,
    #[serde(other)]
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
            Source::Cohere => "cohere",
            Source::Google => "google",
            Source::Meta => "meta",
            Source::Mistral => "mistral",
            Source::Unbabel => "unbabel",
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

/// How far this app is willing to stand behind a catalogue entry.
///
/// One field replacing the two booleans (`supported` / `verified`) that used to
/// answer overlapping halves of the same question. They were never independent:
/// `supported: false` was always accompanied by `verified: false`, because an
/// entry nobody offers cannot have been run. Three states, one axis:
///
/// | status | meaning | solver | download |
/// |---|---|---|---|
/// | [`Supported`](SupportStatus::Supported) | run end to end, works | plans it, ranks it highest | yes |
/// | [`Unverified`](SupportStatus::Unverified) | offered, never exercised here | plans it, ranked below verified | yes |
/// | [`Unsupported`](SupportStatus::Unsupported) | deliberately not offered | never plans it | no |
///
/// The retired case is `sortformer_diar` (v1): it runs, but its encoder graph is
/// fixed-capacity at 120 s of audio and it caps out at four speakers, while
/// `nemotron_3_diar` has neither limit and a permissive licence. Keeping it in
/// the catalogue at `Unsupported` is more honest than deleting it — the
/// measurement that ruled it out stays next to the model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SupportStatus {
    /// Run end to end on this machine; known to work.
    Supported,
    /// Offered, but nobody has exercised it here. Stays selectable — hiding most
    /// of the 模型调查 survey behind a manual override would be worse.
    Unverified,
    /// This app does not offer it at all.
    Unsupported,
}

impl SupportStatus {
    /// Stable lowercase name, matching the `models.json` spelling.
    pub fn as_str(self) -> &'static str {
        match self {
            SupportStatus::Supported => "supported",
            SupportStatus::Unverified => "unverified",
            SupportStatus::Unsupported => "unsupported",
        }
    }

    /// True when the solver may plan it and the downloader may fetch it.
    pub fn is_offered(self) -> bool {
        !matches!(self, SupportStatus::Unsupported)
    }

    /// True when someone has actually run it end to end.
    pub fn is_verified(self) -> bool {
        matches!(self, SupportStatus::Supported)
    }
}

/// The catalogue's default for an entry that predates `status`.
///
/// `unverified` rather than `supported`: the old catalogue carried
/// `verified: true` on five entries out of 129, so "absent means verified" would
/// silently promote the other 124. It is not `unsupported` either — that would
/// empty every recognition plan, and the catalogue is all-or-nothing.
fn default_status() -> SupportStatus {
    SupportStatus::Unverified
}

/// A run mode `audiocpp_cli --mode` accepts for this model.
///
/// Declared per entry from the family's own spec (`modes: ["offline",
/// "streaming"]`), never inferred here — the checkpoint decides what its
/// architecture can do, and the CLI's loader table agrees with it.
///
/// **A declaration is not an entitlement to be used.** Streaming is only
/// selectable for a model this app can actually drive in streaming mode, and
/// that is a narrower set than "declares streaming":
///
/// * the ASR path is `--request-sequence`, which `audiocpp_cli` rejects in any
///   mode but offline (`app/cli/main.cpp`: *batch inputs require offline mode*),
///   so a streaming ASR model still runs offline through this pipeline;
/// * diarization is a single whole-file call and is therefore the one stage that
///   can genuinely stream.
///
/// [`RunMode::is_selectable`] is that predicate, and it is the only place the
/// distinction is decided.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RunMode {
    /// One call over the whole input; the CLI default and the historic behaviour.
    Offline,
    /// The model's own streaming policy: audio fed in chunks, state carried
    /// across them. Lower peak memory, and speaker ids stay consistent because
    /// the session is never restarted.
    Streaming,
}

impl RunMode {
    pub fn as_str(self) -> &'static str {
        match self {
            RunMode::Offline => "offline",
            RunMode::Streaming => "streaming",
        }
    }

    /// True when `audiocpp_cli` would be passed `--mode streaming`.
    pub fn is_streaming(self) -> bool {
        matches!(self, RunMode::Streaming)
    }
}

/// Default for an entry that predates `modes`: offline only.
///
/// Deliberately the narrow answer. `--mode` is never passed for offline (it is
/// the CLI default), so an entry without the key keeps running exactly as it
/// did, and a model whose family does stream is opt-in through the catalogue.
fn default_modes() -> Vec<RunMode> {
    vec![RunMode::Offline]
}

/// One licence attached to a model. `licenses[]` supersedes the single
/// `license` string, which is retained for back-compat.
#[derive(Debug, Clone)]
pub struct License {
    pub name: String,
    pub url: Option<String>,
}

/// One model entry from `models.json`.
///
/// `Default` is hand-written rather than derived so `status` defaults to
/// `Unverified`: a fixture that spells only the fields it cares about
/// (`..Default::default()`) describes an offered model, and a derived `Default`
/// would make every such fixture unsupported — the same trap the wire side
/// avoids with `default_status()`.
#[derive(Debug, Clone)]
pub struct ModelSpec {
    pub id: String,
    /// audio.cpp `family` for recognition models (e.g. `sense_asr`); for a
    /// `llamacpp` translate model it is only the prompt-template hint
    /// (translation-design §9) and never reaches audio.cpp.
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
    /// Which inference engine runs this model; absent means audio.cpp.
    pub engine: Engine,
    /// Prompt-template kind for a translate model (e.g. `hy_mt`), read from
    /// `models.json` so no prompt wording is hardcoded in Rust
    /// (translation-design §4). `None` for every recognition model.
    pub prompt_template: Option<String>,
    pub speed: Option<Speed>,
    pub source: Option<Source>,
    pub tier: Option<Tier>,
    /// How far this app stands behind the entry: supported / unverified /
    /// unsupported (see [`SupportStatus`]).
    ///
    /// Orthogonal to [`Self::tier`]: `tier` says how much we *recommend* a model
    /// within the offered set, this says whether the model is offered at all and
    /// whether anyone has run it. A model that loads is not necessarily a model
    /// that runs — a GGUF can declare a family and still fail on the CLI.
    ///
    /// The two booleans this replaced (`verified` / `supported`) could express a
    /// state that never existed: `supported: false, verified: true` claimed an
    /// entry was measured and also never offered. One enum cannot.
    pub status: SupportStatus,
    /// Run modes this model's own spec declares (`offline` / `streaming`).
    ///
    /// Read from `models.json`, which takes it from the family spec embedded in
    /// the GGUF. Declaring a mode is not the same as offering it here — see
    /// [`RunMode`] — and [`Self::streaming_selectable`] is what the GUI and the
    /// pipeline consult.
    pub modes: Vec<RunMode>,
    pub licenses: Vec<License>,
    /// Other catalogue ids this model cannot work without (v3).
    ///
    /// `qwen3_asr` is the reason this exists: its `timestamps: "word"` claim is
    /// only true when the forced aligner is loaded, and the aligner is a separate
    /// 1.1 GB download. Declaring that in JSON keeps the dependency out of Rust —
    /// the solver turns an uninstalled requirement into a downloadable gap
    /// instead of planning a run that would fail on the CLI.
    pub requires: Vec<String>,
    /// App language tag → the exact token this model expects (v3).
    ///
    /// `qwen3_asr` does not take BCP-47 tags: its prompt is built as
    /// `language <token><asr_text>` and it ships tokens like `Chinese`,
    /// `Cantonese`, `Japanese`. Passing `zh` still transcribes correctly by
    /// accident (measured), but the forced aligner picks CJK tokenisation from
    /// that same token, so the mapping belongs in one declarative place rather
    /// than in a match arm. Untagged entries fall back to `catalog::map_language`.
    pub language_tokens: Vec<LanguageToken>,
    /// Longest audio, in seconds, this model can diarize in **one** session (v4).
    ///
    /// Declared per entry because it is baked into the checkpoint's own config,
    /// not into the code: `sortformer_diar` sizes its encoder graph from
    /// `fc_encoder.max_position_embeddings` / `tf_encoder.max_source_positions`,
    /// which works out to 120 s. Measured on audio.cpp 0.9.1: 120 s succeeds,
    /// 121 s fails with `session_len_sec exceeds tf_encoder.max_source_positions`
    /// — and no `--session-option` can raise it, because the limit is the
    /// architecture, not a budget.
    ///
    /// `None` means "the catalogue declares no ceiling", which keeps the old
    /// single-call behaviour for every model that has never hit one.
    pub max_session_seconds: Option<f64>,
}

/// A `ModelSpec` with nothing filled in but `status: Unverified`, which is what a
/// fixture means when it spells only a few fields.
impl Default for ModelSpec {
    fn default() -> Self {
        Self {
            id: String::new(),
            family: String::new(),
            display_name: String::new(),
            target_directory: String::new(),
            model_file: String::new(),
            files: Vec::new(),
            languages: Vec::new(),
            license: String::new(),
            attribution: None,
            capabilities_raw: None,
            roles: Vec::new(),
            engine: Engine::default(),
            prompt_template: None,
            speed: None,
            source: None,
            tier: None,
            status: SupportStatus::Unverified,
            modes: default_modes(),
            licenses: Vec::new(),
            requires: Vec::new(),
            language_tokens: Vec::new(),
            max_session_seconds: None,
        }
    }
}

/// One `app tag → model token` pair from `models.json`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LanguageToken {
    /// The tag the app uses (`zh`, `en`, `yue`, …).
    pub tag: String,
    /// The token the model expects (`Chinese`, `English`, `Cantonese`, …).
    pub token: String,
}

/// The audio.cpp family whose forced-aligner session options this app knows how
/// to drive ([`crate::engine::aligned`]).
///
/// Word timing is not a generic CLI feature here: `--words-out` is honoured by
/// several families, but the *path* this app implements is qwen3's — it passes
/// `qwen3_asr.forced_aligner_path` and `qwen3_asr.vad_path` as family-scoped
/// session options and needs a companion `align`-role entry for the aligner
/// weights. A family that emits words from its own decoder (parakeet, nemotron,
/// crisperwhisper, gigaam) would need a different invocation, so it must not be
/// planned for a word-timestamp request until that path exists.
///
/// Kept as a constant rather than a `models.json` field because it describes the
/// *runtime*, not the model: the day a second family is wired up, this becomes a
/// list and every word-capable entry starts qualifying on its own.
pub const WORD_ALIGNED_FAMILY: &str = "qwen3_asr";

impl ModelSpec {
    /// Effective capabilities, applying the v1 fallback when the entry did not
    /// declare any (design §4.2).
    pub fn capabilities(&self) -> Capabilities {
        self.capabilities_raw.unwrap_or_default()
    }

    /// True when a word-timestamp request can actually be honoured by *this*
    /// app for this model.
    ///
    /// Two conditions, and both are load-bearing:
    ///
    /// 1. the model declares `timestamps: word` — a claim about the checkpoint;
    /// 2. its family is [`WORD_ALIGNED_FAMILY`] — a claim about the code path.
    ///
    /// The second is why this is not [`ModelSpec::capabilities`]: with the survey
    /// catalogue loaded, `parakeet_tdt` and `nemotron_asr` also declare `word`,
    /// and ranking on the declaration alone made the solver pick them for a
    /// word request — a run that then silently fell back to estimated timing.
    /// Capability that cannot be delivered is worse than no capability, because
    /// the user only finds out in the output.
    pub fn supports_word_timestamps(&self) -> bool {
        self.capabilities().timestamps >= TimestampGrade::Word
            && self.family == WORD_ALIGNED_FAMILY
    }

    /// True when the model can fill `role`; a v1 entry with no `roles` defaults
    /// to `["asr"]`.
    pub fn has_role(&self, role: Role) -> bool {
        let roles = self.effective_roles();
        roles.contains(&role)
    }

    /// True when this app can drive this model in streaming mode.
    ///
    /// Two conditions, and the second is the one that is easy to get wrong:
    ///
    /// 1. the checkpoint declares `streaming` — it is an architecture claim, and
    ///    a model that cannot stream fails at `start_stream()`, not at parse
    ///    time;
    /// 2. the stage that would run it is a **single whole-file call**. Today
    ///    that is diarization only. Recognition goes through
    ///    `--request-sequence`, and `audiocpp_cli` rejects every batch input
    ///    outside offline mode (`batch inputs require offline mode`), so
    ///    offering a streaming switch for `sense_asr` would be a control that
    ///    changes nothing but the error message.
    ///
    /// This is deliberately the single place the rule lives: the GUI shows the
    /// switch from it and the pipeline picks `--mode` from it, so a model cannot
    /// be offered a mode it will not be run in.
    pub fn streaming_selectable(&self) -> bool {
        self.modes.contains(&RunMode::Streaming)
            && self.has_role(Role::Diar)
            && self.engine == Engine::Audiocpp
    }

    /// The mode a run should use when the user has expressed no preference.
    ///
    /// Streaming, when it is available: the app is local-first and runs beside
    /// whatever else the machine is doing, so the lower peak footprint is the
    /// better default. It also fixes a correctness problem rather than only a
    /// memory one — an offline run over a long file is split into windows that
    /// each number speakers from zero, and the stitching that joins them is a
    /// vote, not an identity. A streaming session is never restarted, so its
    /// speaker ids are consistent by construction.
    pub fn default_mode(&self) -> RunMode {
        if self.streaming_selectable() {
            RunMode::Streaming
        } else {
            RunMode::Offline
        }
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
    engine: Engine,
    #[serde(default)]
    prompt_template: Option<String>,
    #[serde(default)]
    speed: Option<Speed>,
    #[serde(default)]
    source: Option<Source>,
    #[serde(default)]
    tier: Option<Tier>,
    /// How far this app stands behind the entry (see [`SupportStatus`]).
    ///
    /// `#[serde(default = "default_status")]` rather than a plain `default`:
    /// the fallback must be `Unverified`, not the enum's first variant, so a
    /// catalogue written before this field existed keeps every entry offered.
    ///
    /// Deliberately **not** `#[serde(other)]`: unlike `source` / `role`, whose
    /// values come from a third-party survey and must never take the catalogue
    /// down, `status` is authored here. A typo has to fail loudly — silently
    /// reading `"supoprted"` as unverified would keep a model we meant to
    /// retire in the plan.
    #[serde(default = "default_status")]
    status: SupportStatus,
    /// Run modes the family declares. Absent means offline only — see
    /// [`default_modes`].
    ///
    /// Not `#[serde(other)]`-tolerant: the values come from the family spec, and
    /// a typo here would silently disable streaming for a model that supports
    /// it, which is exactly the kind of failure nobody would notice.
    #[serde(default = "default_modes")]
    modes: Vec<RunMode>,
    #[serde(default)]
    licenses: Vec<LicenseWire>,
    #[serde(default)]
    requires: Vec<String>,
    #[serde(default)]
    language_tokens: Vec<LanguageTokenWire>,
    /// v4: a hard per-session audio ceiling (see `ModelSpec`).
    #[serde(default)]
    max_session_seconds: Option<f64>,
}

#[derive(Deserialize)]
struct LanguageTokenWire {
    tag: String,
    token: String,
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
            engine: model.engine,
            prompt_template: model.prompt_template,
            speed: model.speed,
            source: model.source,
            tier: model.tier,
            status: model.status,
            modes: model.modes,
            licenses: model
                .licenses
                .into_iter()
                .map(|license| License {
                    name: license.name,
                    url: license.url,
                })
                .collect(),
            requires: model.requires,
            language_tokens: model
                .language_tokens
                .into_iter()
                .map(|pair| LanguageToken {
                    tag: pair.tag,
                    token: pair.token,
                })
                .collect(),
            max_session_seconds: model.max_session_seconds,
        });
    }

    Ok(specs)
}

/// Load the catalogue bundled at `src-tauri/resources/models.json`
/// (dev: repo path; prod: `Resources/resources/models.json`).
pub fn catalog(paths: &AppPaths) -> Result<Vec<ModelSpec>, EngineError> {
    let path = crate::engine::cli::resolve_catalog_path(paths)?;
    load_catalog(&path)
}

/// Absolute model file path for `id`, or `None` when unknown.
pub fn model_path(
    paths: &AppPaths,
    id: &str,
) -> Result<Option<PathBuf>, EngineError> {
    let Some(spec) = lookup(paths, id)? else {
        return Ok(None);
    };
    Ok(Some(
        paths.models_root.join(&spec.target_directory).join(&spec.model_file),
    ))
}

/// `<app_data_dir>/models/<target_directory>`.
pub fn model_dir(paths: &AppPaths, id: &str) -> Result<PathBuf, EngineError> {
    let spec = find(paths, id)?;
    Ok(paths.models_root.join(&spec.target_directory))
}

/// True when every manifest file exists and its size matches (never hashes).
pub fn is_installed(paths: &AppPaths, id: &str) -> bool {
    let Ok(Some(spec)) = lookup(paths, id) else {
        return false;
    };
    let dir = paths.models_root.join(&spec.target_directory);
    spec.files
        .iter()
        .all(|file| file_size(&dir.join(&file.path)) == Some(file.size))
}

/// List every model with its install state; used by `list_models`.
pub fn list(paths: &AppPaths) -> Result<Vec<ModelStatus>, EngineError> {
    let specs = catalog(paths)?;

    Ok(specs
        .into_iter()
        .map(|spec| {
            // The root always exists as a value now (it is a field of `AppPaths`,
            // not an optional resolved at call time), so `installed` starts true
            // and is falsified by any missing or wrong-sized file below.
            let dir = paths.models_root.join(&spec.target_directory);
            let mut installed = true;
            let mut size_on_disk = 0u64;
            for file in &spec.files {
                match file_size(&dir.join(&file.path)) {
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

/// Every catalogue model that can translate, in a deterministic order
/// (catalogue order, so the UI list and the default pick cannot disagree).
///
/// Translation selection is deliberately independent of the recognition
/// `ModelPlan` (translation-design §9): the solver must never see these models.
/// A translate entry is a llama.cpp entry by definition (§6); that is asserted
/// against the committed catalogue rather than re-checked here, so an entry with
/// the role but the wrong engine fails loudly in a test instead of silently
/// disappearing from the picker.
pub fn translation_models(catalogue: &[ModelSpec]) -> Vec<&ModelSpec> {
    catalogue
        .iter()
        .filter(|spec| spec.has_role(Role::Translate))
        .collect()
}

/// The translation model used when the user has not chosen one.
///
/// Ranked by [`Tier`], best first (`default` > `recommended` > `optional` >
/// `experimental`), ties broken by catalogue order so the answer never depends
/// on how the JSON happens to be sorted at runtime. `None` when the catalogue
/// offers no translation model at all, which is what makes translation a
/// no-op rather than an error.
pub fn default_translation_model_id(catalogue: &[ModelSpec]) -> Option<String> {
    translation_models(catalogue)
        .into_iter()
        .min_by_key(|spec| tier_rank(spec.tier))
        .map(|spec| spec.id.clone())
}

/// Sort key for [`default_translation_model_id`]; an undeclared tier ranks last.
fn tier_rank(tier: Option<Tier>) -> u8 {
    match tier {
        Some(Tier::Default) => 0,
        Some(Tier::Recommended) => 1,
        Some(Tier::Optional) => 2,
        Some(Tier::Experimental) => 3,
        None => 4,
    }
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
///
/// A model that declares `language_tokens` (v3) answers with its own token
/// instead: `qwen3_asr` wants `Chinese`/`English`/`Cantonese`, not `zh`/`en`/`yue`.
/// That is not cosmetic — the forced aligner picks its tokenisation (per CJK
/// character vs per space-separated word) from the same string.
pub fn map_language(model: &ModelSpec, app_lang: &str) -> String {
    let tag = normalize_language_tag(app_lang);

    // A declared mapping is authoritative: it exists precisely because the
    // model's own vocabulary differs from the app's tags.
    if let Some(pair) = model.language_tokens.iter().find(|pair| pair.tag == tag) {
        return pair.token.clone();
    }

    // `sense_asr` is the only family whose real tokens are known to be a strict
    // subset of the tags it accepts. Other families (spec §5.6) do map every tag
    // they declare, so their `languages` list is authoritative.
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

// The models root now comes from `AppPaths`, which resolves the same
// precedence `model_dir::configured_root` used to (env → persisted setting →
// default). Keeping it in one field is what stops the catalogue and the
// downloader from disagreeing about where models live.

fn lookup(paths: &AppPaths, id: &str) -> Result<Option<ModelSpec>, EngineError> {
    Ok(catalog(paths)?.into_iter().find(|spec| spec.id == id))
}

fn find(paths: &AppPaths, id: &str) -> Result<ModelSpec, EngineError> {
    lookup(paths, id)?.ok_or_else(|| EngineError::Protocol(format!("unknown model id: {}", id)))
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

    /// The committed catalogue carries ~120 optional models from the survey
    /// spreadsheets, not just the 10 defaults. Three of them used to fail
    /// deserialization (a `slow` speed, a third-party `source`, a `gen` role /
    /// non-C++ engine), and because `load_catalog` is all-or-nothing that took
    /// EVERY model down — including the ones that work.
    ///
    /// These tests pin the enum flexibility that prevents a repeat: an unknown
    /// vendor degrades to `Other`, an unknown engine to `Unknown`, and neither
    /// is ever offered for download or picked by the solver.
    #[test]
    fn unknown_enum_values_degrade_instead_of_failing_the_catalogue() {
        let specs = parse_inline(
            r#"{"models": [
              {
                "id": "vendored", "family": "sense_asr", "display_name": "V",
                "target_directory": "V", "model_file": "v.gguf",
                "files": [{"path": "v.gguf", "url": "https://x/v.gguf", "size": 1}],
                "languages": ["auto"], "license": "MIT",
                "roles": ["asr"], "speed": "slow", "source": "no-such-vendor"
              },
              {
                "id": "musical", "family": "ace_step", "display_name": "M",
                "target_directory": "M", "model_file": "m.gguf",
                "files": [{"path": "m.gguf", "url": "https://x/m.gguf", "size": 1}],
                "languages": ["auto"], "license": "MIT",
                "roles": ["gen"], "engine": "diffusers", "speed": "balanced"
              }
            ]}"#,
        );

        assert_eq!(specs.len(), 2, "one odd entry must not sink the whole file");
        assert_eq!(specs[0].speed, Some(Speed::Slow));
        assert_eq!(
            specs[0].source,
            Some(Source::Other),
            "an unknown vendor is 'no bias', not a parse error"
        );
        assert_eq!(specs[1].engine, Engine::Unknown);
        assert!(specs[1].has_role(Role::Gen));
        assert!(
            !specs[1].engine.is_runnable(),
            "a Python-engine entry must never be offered for download"
        );
        assert!(specs[0].engine.is_runnable());
    }

    /// `slow` must not be conflated with `accurate`: they mean different things
    /// (14B model vs. careful small model), and the cost badge is derived from
    /// the rank.
    #[test]
    fn the_committed_catalogue_carries_slow_and_survey_vendors() {
        let specs = committed_catalog();
        assert!(
            specs.iter().any(|spec| spec.speed == Some(Speed::Slow)),
            "the survey includes 7-14B translation models"
        );
        assert!(
            specs
                .iter()
                .any(|spec| spec.source == Some(Source::Google)),
            "gemma entries carry a real vendor, not Other"
        );
        assert!(
            specs.iter().all(|spec| spec.engine.is_runnable()),
            "the committed file only lists models this build can run"
        );
    }

    /// An entry that says nothing about `status` is **offered but unverified**.
    ///
    /// Both halves matter. `Unsupported` would empty every plan (the catalogue is
    /// all-or-nothing); `Supported` would claim 124 entries nobody has run are
    /// proven, which is the one thing the field exists to prevent.
    #[test]
    fn a_missing_status_means_offered_but_unverified() {
        let specs = parse_inline(
            r#"{"version":2,"models":[
              {"id":"m","family":"sense_asr","display_name":"M","target_directory":"d",
               "model_file":"m.gguf","files":[],"languages":["zh"],"license":"x",
               "capabilities":{"timestamps":"none","diarization":"none","produces_text":true},
               "roles":["asr"]}
            ]}"#,
        );

        assert_eq!(
            specs[0].status,
            SupportStatus::Unverified,
            "absent `status` must neither hide the model nor call it verified"
        );
        assert!(specs[0].status.is_offered());
        assert!(!specs[0].status.is_verified());
    }

    /// An entry that says nothing about `modes` is offline only, and never
    /// streaming.
    ///
    /// The narrow default is the safe one: `--mode` is only emitted for
    /// streaming, so an entry without the key keeps running exactly as it did.
    /// Reading absence as "supports streaming" would push a model whose family
    /// cannot stream into `start_stream()`.
    #[test]
    fn a_missing_modes_key_means_offline_only() {
        let specs = parse_inline(
            r#"{"version":2,"models":[
              {"id":"m","family":"nemotron_3_diar","display_name":"M","target_directory":"d",
               "model_file":"m.gguf","files":[],"languages":["auto"],"license":"x",
               "capabilities":{"timestamps":"turns","diarization":"turns","produces_text":false},
               "roles":["diar"]}
            ]}"#,
        );

        assert_eq!(specs[0].modes, vec![RunMode::Offline]);
        assert!(
            !specs[0].streaming_selectable(),
            "a model that does not declare streaming must not be offered it"
        );
        assert_eq!(specs[0].default_mode(), RunMode::Offline);
    }

    /// The one mode predicate: a declaration alone is not enough, because the
    /// pipeline has to be able to drive it.
    ///
    /// `sense_asr` declares `streaming` — the family really does have a streaming
    /// session — but recognition here is `--request-sequence`, and the CLI
    /// rejects every batch input outside offline mode. Offering the switch would
    /// produce a control whose only effect is a spawn-time error.
    #[test]
    fn only_a_streaming_diar_model_is_actually_run_streaming() {
        let specs = committed_catalog();

        let asr = specs
            .iter()
            .find(|spec| spec.id == "sensevoice_small_q8")
            .expect("the default ASR entry is in the catalogue");
        assert!(
            asr.modes.contains(&RunMode::Streaming),
            "the checkpoint declares streaming, which is why the distinction matters"
        );
        assert!(!asr.streaming_selectable());
        assert_eq!(
            asr.default_mode(),
            RunMode::Offline,
            "recognition runs through --request-sequence, which the CLI only accepts offline"
        );

        let diar = specs
            .iter()
            .find(|spec| spec.id == "nemotron_3_diar_q8_0")
            .expect("the default diar entry is in the catalogue");
        assert!(diar.streaming_selectable());
        assert_eq!(
            diar.default_mode(),
            RunMode::Streaming,
            "no stored preference means the default, and streaming is the default here"
        );

        // The retired v1 entry declares offline only and stays retired.
        for spec in specs.iter().filter(|spec| spec.family == "sortformer_diar") {
            assert!(!spec.streaming_selectable());
            assert_eq!(spec.default_mode(), RunMode::Offline);
        }
    }

    /// The committed file must not offer v1 diarization, and must still offer the
    /// model that replaced it. This is the measurement that drove the switch:
    /// `sortformer_diar`'s graph is fixed at 120 s of audio.
    #[test]
    fn the_committed_catalogue_retires_v1_diarization_for_nemotron() {
        let specs = committed_catalog();

        for spec in specs
            .iter()
            .filter(|spec| spec.family == "sortformer_diar")
        {
            assert_eq!(
                spec.status,
                SupportStatus::Unsupported,
                "{} is still offered, but its 120 s graph ceiling makes it unusable \
                 for real recordings",
                spec.id
            );
            assert_eq!(
                spec.max_session_seconds,
                Some(120.0),
                "the ceiling is the reason it was retired, so it stays recorded"
            );
        }

        let nemotron = specs
            .iter()
            .find(|spec| spec.id == "nemotron_3_diar_q8_0")
            .expect("the replacement is in the catalogue");
        assert!(nemotron.status.is_offered());
        assert_eq!(
            nemotron.max_session_seconds, None,
            "Nemotron has no fixed position table, so it declares no ceiling"
        );
        assert_eq!(
            nemotron.tier,
            Some(Tier::Default),
            "the default diar model is the one that works on long audio"
        );
    }

    /// Parse an in-memory catalogue document (the crate has no `tempfile`).
    fn parse_inline(document: &str) -> Vec<ModelSpec> {
        let temp = std::env::temp_dir().join(format!(
            "monosub-catalog-inline-{}-{}.json",
            std::process::id(),
            uuid::Uuid::new_v4()
        ));
        std::fs::write(&temp, document).expect("write inline catalogue");
        let specs = load_catalog(&temp).expect("inline catalogue parses");
        std::fs::remove_file(&temp).expect("remove inline catalogue");
        specs
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
    fn load_catalog_parses_v2_capability_fields() {        let specs = committed_catalog();
        let moss = specs
            .iter()
            .find(|spec| spec.id == "moss_transcribe_diarize_q8_0")
            .expect("models.json declares moss_transcribe_diarize_q8_0");

        let caps = moss.capabilities();
        assert_eq!(caps.timestamps, TimestampGrade::Segment);
        assert_eq!(caps.diarization, DiarizationGrade::TurnsWithText);
        assert!(caps.produces_text);
        // No `passthrough` role any more: MOSS runs the same VAD-segmented
        // pipeline as every other ASR model (docs/tech-spec/audiocpp-moss-poc.md §5).
        assert!(!moss.has_role(Role::Passthrough));
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

    /// The aligner is a first-class entry (v3): `qwen3_asr` cannot produce word
    /// timestamps without it, so the solver must be able to name it as a
    /// download. It is diar-less and text-less, and only fills `align`.
    ///
    /// Replaces the old `catalogue_has_no_align_only_model` guard: that guard
    /// encoded the (wrong) conclusion that no aligner entry was needed at all.
    /// What is still forbidden is planning an `align` *step*, which the solver
    /// enforces separately — the aligner runs inside the ASR session.
    #[test]
    fn catalogue_declares_the_forced_aligner_as_qwen3_asr_dependency() {
        let specs = committed_catalog();
        let aligner = specs
            .iter()
            .find(|spec| spec.id == "qwen3_forced_aligner_0_6b_q8_0")
            .expect("the forced aligner is a catalogue entry");

        assert_eq!(aligner.family, "qwen3_forced_aligner");
        assert!(aligner.has_role(Role::Align));
        assert_eq!(aligner.capabilities().timestamps, TimestampGrade::Word);
        assert!(
            !aligner.capabilities().produces_text,
            "the aligner aligns an existing transcript; it never writes one"
        );
        assert_eq!(
            aligner.files[0].sha256.as_deref(),
            Some("75209490b11cec2b0db749ca5f4ff92266f58efd30f7fd04d9eb2a3ac9cc929f"),
            "pinned hash of the measured upstream artifact"
        );

        let asr = specs
            .iter()
            .find(|spec| spec.id == "qwen3_asr_0_6b_q8_0")
            .expect("qwen3_asr entry present");
        assert_eq!(
            asr.requires,
            vec!["qwen3_forced_aligner_0_6b_q8_0".to_string()],
            "the word-timestamp claim is conditional on the aligner"
        );
    }

    /// `map_language` must answer with the model's own token when the catalogue
    /// declares one. Measured on v0.9.1: `zh` still transcribes correctly, but
    /// the forced aligner chooses CJK tokenisation from this same string, so
    /// forwarding the bare tag silently changes how words are split.
    #[test]
    fn declared_language_tokens_win_over_tag_forwarding() {
        let specs = committed_catalog();
        let asr = specs
            .iter()
            .find(|spec| spec.id == "qwen3_asr_0_6b_q8_0")
            .expect("qwen3_asr entry present");

        assert_eq!(map_language(asr, "zh"), "Chinese");
        assert_eq!(map_language(asr, "en"), "English");
        assert_eq!(map_language(asr, "yue"), "Cantonese");
        assert_eq!(map_language(asr, "ja"), "Japanese");
        // `zh-tw` normalises to `zh` first, so it inherits the mapping.
        assert_eq!(map_language(asr, "zh-tw"), "Chinese");
        // A tag with no declared token falls back to the documented rule: a tag
        // the model lists is forwarded, anything else is `auto`.
        assert_eq!(map_language(asr, "auto"), "auto");

        // Families without `language_tokens` are untouched (regression guard).
        let sense = sensevoice_spec();
        assert_eq!(map_language(&sense, "zh"), "zh");
        assert_eq!(map_language(&sense, "yue"), "yue");
        assert_eq!(
            map_language(&sense, "es"),
            "auto",
            "sense_asr accepts the tag but only tokens a subset"
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

    /// A translate entry parses with its own engine and prompt template, while
    /// every existing entry keeps the audio.cpp default when `engine` is absent
    /// (translation-design §6, §9).
    #[test]
    fn load_catalog_parses_engine_and_prompt_template() {
        let with_fields = parse_inline(
            r#"{
              "version": 2,
              "models": [
                {
                  "id": "hy_mt",
                  "family": "hy_mt",
                  "display_name": "HY-MT",
                  "target_directory": "HY-MT1.5-1.8B-GGUF",
                  "model_file": "HY-MT1.5-1.8B-Q4_K_M.gguf",
                  "files": [],
                  "languages": ["zh", "en"],
                  "license": "Tencent HY Community License Agreement",
                  "roles": ["translate"],
                  "engine": "llamacpp",
                  "prompt_template": "hy_mt"
                }
              ]
            }"#,
        );

        assert_eq!(with_fields[0].engine, Engine::Llamacpp);
        assert_eq!(with_fields[0].engine.as_str(), "llamacpp");
        assert_eq!(with_fields[0].prompt_template.as_deref(), Some("hy_mt"));
        assert_eq!(with_fields[0].roles, vec![Role::Translate]);
        assert_eq!(Role::Translate.as_str(), "translate");
        assert!(with_fields[0].has_role(Role::Translate));

        let without = parse_inline(
            r#"{
              "version": 2,
              "models": [
                {
                  "id": "sense",
                  "family": "sense_asr",
                  "display_name": "Sense",
                  "target_directory": "d",
                  "model_file": "m.gguf",
                  "files": [],
                  "languages": ["auto"],
                  "license": "MIT"
                }
              ]
            }"#,
        );

        assert_eq!(without[0].engine, Engine::Audiocpp);
        assert_eq!(Engine::default().as_str(), "audiocpp");
        assert_eq!(without[0].prompt_template, None);
        assert!(!without[0].has_role(Role::Translate));
    }

    /// The committed catalogue must carry a usable translate entry: a
    /// llama.cpp engine, a prompt template, and no recognition role, so the
    /// solver can never pick it (translation-design §9).
    #[test]
    fn committed_translate_entry_is_llamacpp_and_outside_recognition() {
        let specs = committed_catalog();
        let translate = translation_models(&specs);
        assert!(
            !translate.is_empty(),
            "models.json must declare at least one translate model"
        );

        for spec in translate {
            assert_eq!(
                spec.engine,
                Engine::Llamacpp,
                "{} must run on llama.cpp",
                spec.id
            );
            assert!(
                spec.prompt_template.is_some(),
                "{} needs a prompt template kind",
                spec.id
            );
            assert!(
                !spec.has_role(Role::Asr)
                    && !spec.has_role(Role::Passthrough)
                    && !spec.has_role(Role::Diar),
                "{} must not be selectable for recognition",
                spec.id
            );
        }

        // Recognition entries stay on audio.cpp. The committed v1-shaped entry
        // (written before `engine` existed) must keep parsing as audiocpp.
        assert!(specs
            .iter()
            .filter(|spec| !spec.has_role(Role::Translate))
            .all(|spec| spec.engine == Engine::Audiocpp));
    }

    /// `translation_models` keeps catalogue order, so the picker and the default
    /// cannot disagree about which entry is "first".
    #[test]
    fn translation_models_preserves_catalogue_order() {
        let specs = parse_inline(
            r#"{
              "version": 2,
              "models": [
                {
                  "id": "first", "family": "f", "display_name": "First",
                  "target_directory": "d", "model_file": "m.gguf", "files": [],
                  "languages": [], "license": "MIT", "roles": ["translate"],
                  "engine": "llamacpp", "tier": "optional"
                },
                {
                  "id": "asr", "family": "sense_asr", "display_name": "ASR",
                  "target_directory": "d", "model_file": "m.gguf", "files": [],
                  "languages": [], "license": "MIT", "roles": ["asr"]
                },
                {
                  "id": "second", "family": "f", "display_name": "Second",
                  "target_directory": "d", "model_file": "m.gguf", "files": [],
                  "languages": [], "license": "MIT", "roles": ["translate"],
                  "engine": "llamacpp", "tier": "recommended"
                }
              ]
            }"#,
        );

        let ids: Vec<&str> = translation_models(&specs)
            .into_iter()
            .map(|spec| spec.id.as_str())
            .collect();
        assert_eq!(ids, vec!["first", "second"]);
    }

    /// The default translation pick is the best tier, ties broken by catalogue
    /// order — never by install state, which is a download prompt, not a filter.
    #[test]
    fn default_translation_model_prefers_tier_then_order() {
        let specs = parse_inline(
            r#"{
              "version": 2,
              "models": [
                {
                  "id": "experimental", "family": "f", "display_name": "E",
                  "target_directory": "d", "model_file": "m.gguf", "files": [],
                  "languages": [], "license": "MIT", "roles": ["translate"],
                  "tier": "experimental"
                },
                {
                  "id": "recommended_a", "family": "f", "display_name": "RA",
                  "target_directory": "d", "model_file": "m.gguf", "files": [],
                  "languages": [], "license": "MIT", "roles": ["translate"],
                  "tier": "recommended"
                },
                {
                  "id": "recommended_b", "family": "f", "display_name": "RB",
                  "target_directory": "d", "model_file": "m.gguf", "files": [],
                  "languages": [], "license": "MIT", "roles": ["translate"],
                  "tier": "recommended"
                }
              ]
            }"#,
        );
        assert_eq!(
            default_translation_model_id(&specs).as_deref(),
            Some("recommended_a"),
            "a tier tie keeps catalogue order"
        );

        // A `default` tier outranks `recommended` wherever it sits.
        let mut specs = specs;
        specs[2].tier = Some(Tier::Default);
        assert_eq!(
            default_translation_model_id(&specs).as_deref(),
            Some("recommended_b")
        );

        // No translate model at all: translation is a no-op, not an error.
        assert_eq!(translation_models(&[]).len(), 0);
        assert_eq!(default_translation_model_id(&[]), None);
        let recognition_only = parse_inline(
            r#"{
              "version": 2,
              "models": [
                {
                  "id": "asr", "family": "sense_asr", "display_name": "A",
                  "target_directory": "d", "model_file": "m.gguf", "files": [],
                  "languages": [], "license": "MIT", "roles": ["asr"]
                }
              ]
            }"#,
        );
        assert_eq!(default_translation_model_id(&recognition_only), None);
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
            engine: Engine::default(),
            prompt_template: None,
            speed: None,
            source: None,
            tier: None,
            status: crate::engine::catalog::SupportStatus::Unverified,
            modes: Vec::new(),
            licenses: Vec::new(),
            requires: Vec::new(),
            language_tokens: Vec::new(),
            max_session_seconds: None,
        };

        assert_eq!(map_language(&model, "es"), "es");
        assert_eq!(map_language(&model, "zh-tw"), "zh");
        assert_eq!(map_language(&model, "de"), "auto");
    }
}
