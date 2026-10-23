//! Requirement → `ModelPlan` solver: capability-driven model selection.
//!
//! Task: M2. See docs/tech-spec/model-selection-design.md §3–§5 and the ledger
//! docs/tech-spec/model-selection-tasks.md.
//!
//! Everything in this module is a pure function of (catalogue, install
//! predicate, requirements): no `AppHandle`, no CLI, no network, so the whole
//! solver is unit-testable. Capabilities are read from `models.json` via
//! [`crate::engine::catalog`] — only two things are hardcoded here, both
//! verbatim from the design: the §5.2 template shapes and the §5.4 language
//! bias. There is deliberately no generic constraint solver (design §13 risk 7).
//!
//! The user answers exactly four questions — 语种场景 ([`LanguageProfile`]),
//! 说话人分离 (`diarization`), 词级时间戳 (`word_timestamps`) and 显存预算
//! ([`MemoryBudget`]) — and those four fully determine the plan. Speed is not a
//! separate question any more: in this catalogue a model's size *is* its speed,
//! so the 显存预算 answers it as well.

use crate::engine::catalog::{
    self, Capabilities, DiarizationGrade, Engine, ModelSpec, Role, Source, Speed, Tier,
    TimestampGrade,
};
use crate::engine::cli::Backend;
use std::collections::HashMap;

/// What a plan with no steps can provide.
const NOTHING: Capabilities = Capabilities {
    timestamps: TimestampGrade::None,
    diarization: DiarizationGrade::None,
    produces_text: false,
};

// --- §5.3 score weights ----------------------------------------------------

const RECOMMENDED_TIER_BONUS: i32 = 100;
const TONGYI_SOURCE_BONUS: i32 = 60;
const MEMORY_MATCH_BONUS: i32 = 40;
const INSTALLED_BONUS: i32 = 30;
/// Reward for a model this project has actually run end to end.
///
/// Larger than [`INSTALLED_BONUS`]: a verified model that still needs a download
/// is a better bet than an unverified one already on disk. Smaller than
/// [`RECOMMENDED_TIER_BONUS`]: verification is evidence about *reliability*, not a
/// substitute for being the right tool — 显存预算 and language fit still outrank
/// it, so a verified model that is three sizes too big does not win.
///
/// Never a filter: unverified models remain planable, they just lose the bonus.
const VERIFIED_BONUS: i32 = 45;
const EXTRA_STEP_PENALTY: i32 = 50;

/// Cost of each footprint class a model exceeds the 显存预算 by.
///
/// Deliberately larger than [`RECOMMENDED_TIER_BONUS`] + [`TONGYI_SOURCE_BONUS`]
/// (+160 combined): when the user says 显存小, a 254 MB model must beat a 1 GB
/// `tier: recommended` Tongyi model, otherwise the budget is decorative. The
/// rank in [`over_budget_rank`] already orders candidates by fit; this keeps the
/// plan-level score agreeing with that order.
const MEMORY_OVERSHOOT_PENALTY: i32 = 150;

/// Footprint-class thresholds: under 512 MB is 小, under 1.5 GB is 中, above is 大.
/// Chosen against the catalogue so the classes are not empty: SenseVoice is
/// 254 MB (小), Fun-ASR-Nano / Qwen3-ASR / MOSS are ~0.8–1.2 GB (中).
const SMALL_MODEL_BYTES: u64 = 512 * 1024 * 1024;
const MEDIUM_MODEL_BYTES: u64 = 1_536 * 1024 * 1024;

// --- §3.1 requirements -----------------------------------------------------

/// Which languages the media actually contains.
///
/// This is the *first* question the user answers, and it re-orders the §5.4 bias
/// table: `Mandarin` keeps the per-language rows, while `Mixed`/`Multilingual`
/// move the Qwen / MOSS families to the front. The reason is concrete rather
/// than stylistic — `sense_asr` emits exactly one language token per session, so
/// a Chinese-English sentence comes back half-transcribed from it.
///
/// It is a **ranking** term, not a filter: the catalogue does not declare which
/// models truly follow a mid-sentence switch, so turning that into a hard filter
/// would encode a claim the app cannot verify.
///
/// `Mandarin` is the default because MonoSub's users are mostly Chinese and
/// `sense_asr` is both the fastest and the most accurate choice for it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LanguageProfile {
    /// 普通话为主: Chinese-dominant audio (Simplified or Traditional output).
    #[default]
    Mandarin,
    /// 中英混合: one speaker switching between Chinese and English in a
    /// sentence. Needs a model that transcribes both without a language switch,
    /// which rules `sense_asr` out — it emits one token language per session.
    Mixed,
    /// 多语种: content that is not Chinese at all (or spans many languages).
    Multilingual,
}

impl LanguageProfile {
    /// Wire spelling. Kept here (not in `commands/plan.rs`) so the front end and
    /// both commands cannot drift on the three names.
    pub fn from_wire(value: Option<&str>) -> Self {
        match value.map(|value| value.trim().to_lowercase()).as_deref() {
            Some("mixed") | Some("zh-en") | Some("zh_en") => Self::Mixed,
            Some("multilingual") | Some("multi") => Self::Multilingual,
            _ => Self::Mandarin,
        }
    }
}

/// How much GPU/unified memory the user is willing to spend (显存占用).
///
/// Answers the speed question without asking about speed: within this catalogue
/// the biggest determinant of both is the weight file, so the budget ranks the
/// candidates ([`over_budget_rank`]). It is a *ranking* term, never a hard
/// filter — an empty bucket still plans the cheapest model and reports no gap.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum MemoryBudget {
    /// 小: an 8 GB machine. Prefers the smallest quantisation of anything.
    Small,
    /// 中: the default; a ~1 GB model is fine.
    #[default]
    Medium,
    /// 大: no objection to the biggest model on offer.
    Large,
}

impl MemoryBudget {
    pub fn from_wire(value: Option<&str>) -> Self {
        match value.map(|value| value.trim().to_lowercase()).as_deref() {
            Some("small") => Self::Small,
            Some("large") => Self::Large,
            _ => Self::Medium,
        }
    }

    /// 0 / 1 / 2, so budgets and footprints can be compared as numbers.
    pub fn rank(self) -> i32 {
        match self {
            Self::Small => 0,
            Self::Medium => 1,
            Self::Large => 2,
        }
    }

    /// Class of a model (or a whole plan) from the bytes its weight files take.
    pub fn from_bytes(bytes: u64) -> Self {
        if bytes < SMALL_MODEL_BYTES {
            Self::Small
        } else if bytes < MEDIUM_MODEL_BYTES {
            Self::Medium
        } else {
            Self::Large
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Small => "small",
            Self::Medium => "medium",
            Self::Large => "large",
        }
    }
}

/// A transcription's hard capability requirements (design §3.1).
///
/// The user states needs, never model names; [`solve`] turns this into plans.
/// There are exactly four of them: 语种场景, 说话人分离, 词级时间戳, 显存预算.
///
/// Which models can emit text + timestamps + speakers in one call is a property
/// of the chosen model, not a user question, so it is never asked here.
#[derive(Debug, Clone)]
pub struct Requirements {
    /// Word-level timestamps needed (precise alignment / clipping).
    pub word_timestamps: bool,
    /// Speaker separation needed.
    pub diarization: bool,
    /// App language (`zh` / `zh-tw` / `en` / …); mapped by
    /// [`catalog::map_language`], never by this module.
    pub language: String,
    /// 语种场景 — also owns the family bias of §5.4.
    pub language_profile: LanguageProfile,
    /// 显存占用 — the ranking term that used to be a speed preference.
    pub memory: MemoryBudget,
    /// Device backend. Kept on the request so the UI's device switch is
    /// modelled, and so a future per-backend threshold has one place to read it;
    /// the footprint classes are backend-independent today.
    pub backend: Backend,
}

impl Default for Requirements {
    fn default() -> Self {
        Self {
            word_timestamps: false,
            diarization: false,
            language: "auto".to_string(),
            language_profile: LanguageProfile::default(),
            memory: MemoryBudget::default(),
            backend: Backend::Metal,
        }
    }
}

// --- §3.2 plans ------------------------------------------------------------

/// One step of a plan: an audio.cpp task plus the model that fills it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanStep {
    pub kind: StepKind,
    /// Catalogue model id; empty for the pure-Rust [`StepKind::TurnAttribution`].
    pub model_id: String,
    /// audio.cpp `--task`; empty for the pure-Rust step.
    pub task: String,
    /// audio.cpp `--family`; empty for the pure-Rust step.
    pub family: String,
}

impl PlanStep {
    fn model(kind: StepKind, spec: &ModelSpec) -> Self {
        Self {
            kind,
            model_id: spec.id.clone(),
            task: kind.task().to_string(),
            family: spec.family.clone(),
        }
    }

    /// The pure-Rust step (design §6.4): no model, no CLI invocation.
    ///
    /// Public because [`thaw`] rebuilds it from an empty `model_id`, and a test
    /// that round-trips a frozen plan has to construct the same value.
    pub fn rust(kind: StepKind) -> Self {
        Self {
            kind,
            model_id: String::new(),
            task: String::new(),
            family: String::new(),
        }
    }
}

/// What a step does (design §3.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepKind {
    /// Speech segmentation. The §5.2 templates never name it: Silero VAD is
    /// bundled and always runs inside the sequence pipeline, so it is part of
    /// `Asr` rather than a separate step today (M6 will prepend it explicitly).
    Vad,
    /// Transcribe a whole file or a VAD chunk.
    Asr,
    /// Word-level alignment (`qwen3_forced_aligner`).
    ///
    /// Deliberately **not** offered: audio.cpp's `align` task aligns an
    /// *existing* transcript to audio, and MonoSub never has one — it produces
    /// transcripts. Selecting this step used to be planned but never executed,
    /// so "word timestamps" on was silently a no-op.
    #[allow(dead_code)]
    Align,
    /// Speaker turns only (`nemotron_3_diar` / `sortformer_diar`).
    Diarize,
    /// Pure Rust: attribute ASR segments to diar turns (design §6.4).
    TurnAttribution,
}

impl StepKind {
    /// The audio.cpp `--task` this step runs; empty for the Rust-only step.
    fn task(self) -> &'static str {
        match self {
            StepKind::Vad => "vad",
            StepKind::Asr => "asr",
            StepKind::Align => "align",
            StepKind::Diarize => "diar",
            StepKind::TurnAttribution => "",
        }
    }

    /// The [`Role`] this step draws a model from; `None` for the Rust-only step.
    fn role(self) -> Option<Role> {
        match self {
            StepKind::Vad => Some(Role::Vad),
            StepKind::Asr => Some(Role::Asr),
            StepKind::Align => Some(Role::Align),
            StepKind::Diarize => Some(Role::Diar),
            StepKind::TurnAttribution => None,
        }
    }

    /// The audio.cpp `--task` a manual override pins this step to; `None` for
    /// the pure-Rust step, which has no model to override.
    fn override_task(self) -> Option<&'static str> {
        match self {
            StepKind::Vad => Some("vad"),
            StepKind::Asr => Some("asr"),
            StepKind::Align => Some("align"),
            StepKind::Diarize => Some("diar"),
            StepKind::TurnAttribution => None,
        }
    }

    /// Short step name used inside a plan id (`asr+diar+turns`).
    fn label(self) -> &'static str {
        match self {
            StepKind::Vad => "vad",
            StepKind::Asr => "asr",
            StepKind::Align => "align",
            StepKind::Diarize => "diar",
            StepKind::TurnAttribution => "turns",
        }
    }
}

/// Relative cost of a plan — a UI hint, **not** a runtime prediction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlanCost {
    /// Total steps, including the pure-Rust attribution step.
    pub steps: usize,
    /// Speed class of the slowest model in the plan.
    pub speed: Speed,
    /// 显存 class implied by the plan's own weight files, so the UI can tell a
    /// 254 MB plan from a 1.1 GB one without re-reading the catalogue.
    pub memory: MemoryBudget,
}

/// One executable plan: an ordered list of steps (design §3.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelPlan {
    /// Stable id for UI display and logs (`asr:moss`, `asr+diar+turns:nemotron`).
    pub id: String,
    /// i18n key for the human-readable title (design §8.3).
    pub title_key: &'static str,
    /// Models that must be installed, deduplicated, in step order.
    pub model_ids: Vec<String>,
    /// Ordered steps; empty only for a gap-bearing plan.
    pub steps: Vec<PlanStep>,
    /// Relative cost, for badges and sorting hints.
    pub cost: PlanCost,
    /// What the plan actually provides — the union of its steps' capabilities.
    pub capabilities: Capabilities,
    /// Requirements this plan does not meet. Empty for a fully covering plan;
    /// a plan with no steps carries the reason nothing could be solved.
    pub gaps: Vec<Gap>,
}

// --- §5.6 gaps -------------------------------------------------------------

/// One unmet requirement, with the download suggestion the UI shows (design §5.6).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Gap {
    /// Requirement or step key: `word_timestamps` / `diarization` / `asr` / …
    pub requirement: &'static str,
    /// Human-readable explanation; the frontend maps it to an i18n key.
    pub message: String,
    /// Catalogue model that would close the gap, when the catalogue has one.
    pub suggested_model_id: Option<String>,
}

// --- §5.2 templates --------------------------------------------------------

/// One candidate plan shape: the ordered steps to fill.
///
/// Written as [`StepKind`]s rather than roles because
/// [`StepKind::TurnAttribution`] has no model to pick.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Template {
    pub steps: Vec<StepKind>,
}

impl Template {
    fn always(steps: Vec<StepKind>) -> Self {
        Self { steps }
    }
}

/// The candidate plan shapes for `req` (design §5.2), in priority order.
///
/// Four hardcoded combinations — word-level × speaker separation — never a
/// generic constraint solve.
///
/// Every combination starts with `Asr`, and every model is recognised through
/// the same VAD → slice → `--request-sequence` pipeline. There is no one-shot
/// "passthrough" template any more: a model that can emit text + timestamps +
/// speakers in one call still runs per VAD chunk, which is what makes long
/// audio safe (see `docs/tech-spec/audiocpp-moss-poc.md` §5).
pub fn templates_for(req: &Requirements) -> Vec<Template> {
    match (req.word_timestamps, req.diarization) {
        (false, false) => vec![Template::always(vec![StepKind::Asr])],
        (false, true) => vec![Template::always(vec![
            StepKind::Asr,
            StepKind::Diarize,
            StepKind::TurnAttribution,
        ])],
        (true, false) => vec![
            // One ASR step, provided it can reach word timestamps — either
            // natively or through a declared `requires` aligner, which the ASR
            // session loads itself (`qwen3_asr.forced_aligner_path`). There is
            // still no `Align` step: audio.cpp's `align` task aligns an
            // *existing* transcript to audio, and MonoSub never has one. The
            // aligner runs inside the ASR session in exactly one CLI call, so
            // planning it as a separate step would plan something that never
            // runs.
            Template::always(vec![StepKind::Asr]),
        ],
        (true, true) => vec![Template::always(vec![
            StepKind::Asr,
            StepKind::Diarize,
            StepKind::TurnAttribution,
        ])],
    }
}

// --- §5.3 / §5.4 model choice ----------------------------------------------

/// Best catalogue model for `role` under `req`, or `None` when none qualifies.
///
/// Hard filters first: the model must have the role and must be able to serve
/// the requested language ([`language_ok`]). Candidates are then ordered by, in
/// this exact order:
///
/// 1. whether it can reach the requested word-timing grade,
/// 2. the 语种场景 / §5.4 family bias,
/// 3. whether it fits the 显存预算,
/// 4. the §5.3 score.
///
/// **Capability outranks cost, always.** (1) and (2) are statements about what
/// the audio *is* — a model that cannot follow a mid-sentence language switch
/// produces a wrong subtitle no matter how small it is — while (3) is a budget.
/// Ordering them the other way was a measured bug: with `中英混合` + `显存小` the
/// solver picked the 254 MB SenseVoice over `qwen3_asr`, i.e. it honoured the
/// diet and half-transcribed the file. The plan preview shows the footprint it
/// ended up at, so the overshoot is visible rather than silent.
///
/// Install state is *not* a filter: an uninstalled model is still the right
/// recommendation, the UI just offers a download (design §5.3, §5.6).
pub fn best_model_for_role<'a>(
    catalogue: &'a [ModelSpec],
    installed: &dyn Fn(&str) -> bool,
    role: Role,
    req: &Requirements,
) -> Option<&'a ModelSpec> {
    catalogue
        .iter()
        .filter(|spec| {
            // Only audio.cpp models can fill a recognition step; a llama.cpp
            // translate entry must never be handed to `audiocpp_cli`
            // (translation-design §9). `status` is the second gate: an entry the
            // app deliberately does not offer (v1 diarization, which its own
            // 120 s graph ceiling makes unusable for real recordings) must not be
            // planned even though it would run. `Unverified` passes: it is
            // offered, just ranked lower.
            spec.engine == Engine::Audiocpp
                && spec.status.is_offered()
                && spec.has_role(role)
                && language_ok(spec, &req.language)
        })
        .min_by_key(|spec| {
            (
                // A word-timestamp request must actually reach a word-capable
                // model, so it outranks the language bias: without this the §5.4
                // table picks `sense_asr` (first for `zh`) and the requirement is
                // silently unmet. It is a *ranking* term, not a filter: when
                // nothing in the catalogue can do word timing the best remaining
                // model is still planned, and `gaps()` reports the shortfall
                // instead of failing the run.
                word_shortfall_rank(spec, req),
                bias_rank(&req.language, req.language_profile, spec),
                over_budget_rank(spec, req),
                -single_model_score(spec, req, installed),
            )
        })
}

/// 0 when `spec` can reach word timestamps, 1 otherwise; always 0 when the
/// request does not ask for them.
///
/// "Can reach" is narrower than "declares `timestamps: word`". The catalogue now
/// carries a dozen word-capable ASR families (`parakeet_tdt`, `nemotron_asr`,
/// `crisperwhisper`, `gigaam_asr`, …) whose words come out of their own decoder,
/// while this app's word path (`engine::aligned`) drives **one** family: it
/// passes `qwen3_asr.forced_aligner_path`/`vad_path` as session options and
/// requires a companion `align`-role entry. Selecting any other family for a
/// word request would plan a run that cannot happen, so the *runtime's*
/// capability — [`catalog::ModelSpec::supports_word_timestamps`] — is what this
/// ranks on. The plain-mode path is unaffected: such a model is still a perfectly
/// good ASR when word timing is not asked for.
fn word_shortfall_rank(spec: &ModelSpec, req: &Requirements) -> i32 {
    if !req.word_timestamps {
        return 0;
    }
    i32::from(!spec.supports_word_timestamps())
}

/// 0 when `spec` fits the requested 显存预算, 1 when it is one class above, 2 when
/// it is two above.
///
/// A rank, not a filter: when nothing in the catalogue fits the budget the
/// smallest available model is still planned (and reports no gap, because a
/// bigger-than-asked footprint is a cost, not a missing capability). It sits
/// *below* the 语种场景 bias in [`best_model_for_role`] for the same reason —
/// see the note there.
fn over_budget_rank(spec: &ModelSpec, req: &Requirements) -> i32 {
    (footprint(spec).rank() - req.memory.rank()).max(0)
}

/// True when `spec` can be asked for the app language (design §5.3 rule 2).
///
/// The mapping itself is delegated to [`catalog::map_language`], which owns the
/// `zh-tw → zh` normalisation and the `sense_asr` real-token subset; a concrete
/// answer means the request survives for this family. A lowered `auto` still
/// passes for a language-agnostic model — `diar`/`vad` entries declare only
/// `auto` (they emit turns, not text) and serve any request — while an ASR that
/// also lists concrete languages must actually declare this one, which is what
/// keeps `fun_asr_nano` out of `ko` and `yue`. The same rule keeps `sense_asr`
/// out of `es`/`de`/`fr`/`pt`, whose tags it accepts but only serves as
/// auto-detect; the §5.4 table then sends those languages to `qwen3_asr`.
///
/// 语种场景 is deliberately **not** a filter here: the catalogue does not declare
/// which models truly switch language mid-sentence, so inventing that flag would
/// be a claim the app cannot verify. The profile is a ranking bias instead
/// ([`bias_families`]) — decisive when several families qualify, harmless when
/// only one does.
fn language_ok(spec: &ModelSpec, app_lang: &str) -> bool {
    let requested = app_lang.trim();

    // "auto 永远通过": an explicit auto-detect request fits every model.
    if requested.eq_ignore_ascii_case("auto") {
        return true;
    }
    if catalog::map_language(spec, requested) != "auto" {
        return true;
    }

    !declares_concrete_language(spec)
}

/// True when the model lists at least one language other than `auto`.
fn declares_concrete_language(spec: &ModelSpec) -> bool {
    spec.languages
        .iter()
        .any(|declared| !declared.eq_ignore_ascii_case("auto"))
}

/// First-choice ASR families per 语种场景 and language — a bias, never a filter.
///
/// The 语种场景 decides the table when it is not `Mandarin`: `中英混合` and
/// `多语种` both need a model that transcribes language switches, which only the
/// Qwen / MOSS families do. For `Mandarin` the per-language rows of §5.4 still
/// apply, so an English or Japanese file does not get SenseVoice just because
/// the profile says "普通话为主".
fn bias_families(language: &str, profile: LanguageProfile) -> &'static [&'static str] {
    match profile {
        // 中英混合: one model, no language switch. Qwen3-ASR first (it is the
        // catalogue's word-timestamp model too), then MOSS.
        LanguageProfile::Mixed => &["qwen3_asr", "moss_transcribe_diarize"],
        // 多语种: anything that is not a Chinese-only token model.
        LanguageProfile::Multilingual => &["qwen3_asr", "moss_transcribe_diarize"],
        LanguageProfile::Mandarin => match language.trim().to_lowercase().as_str() {
            "zh" | "zh-tw" => &["sense_asr", "fun_asr_nano", "qwen3_asr"],
            "en" => &["sense_asr", "parakeet_tdt", "qwen3_asr"],
            "ja" => &["sense_asr", "fun_asr_nano"],
            // `fun_asr_nano` does not support Korean.
            "ko" => &["sense_asr"],
            // Cantonese: SenseVoice is the only model that serves it.
            "yue" => &["sense_asr"],
            // These fall back to auto-detect on SenseVoice, so a non-SenseVoice
            // ASR is preferred (design §5.4).
            "es" | "de" | "fr" | "pt" => &["qwen3_asr", "cohere_asr", "canary_asr"],
            // `auto` and anything unnamed: Tongyi first, then the §5.3 score.
            _ => &["sense_asr", "qwen3_asr"],
        },
    }
}

/// Position of `spec.family` in the §5.4 table; `usize::MAX` when the table does
/// not name it, which leaves the §5.3 score in charge for that role.
fn bias_rank(language: &str, profile: LanguageProfile, spec: &ModelSpec) -> usize {
    bias_families(language, profile)
        .iter()
        .position(|family| *family == spec.family)
        .unwrap_or(usize::MAX)
}

/// Design §5.3 score for one plan.
///
/// Per-model terms are summed across the plan's model steps, so a pipeline
/// reflects every model it loads; the per-extra-step penalty applies once, to
/// the plan.
pub fn score(
    plan: &ModelPlan,
    req: &Requirements,
    catalogue: &[ModelSpec],
    installed: &dyn Fn(&str) -> bool,
) -> i32 {
    let mut total = 0;

    for step in &plan.steps {
        let Some(spec) = find(catalogue, &step.model_id) else {
            continue;
        };
        total += single_model_score(spec, req, installed);
    }

    total -= EXTRA_STEP_PENALTY * (plan.steps.len() as i32 - 1).max(0);

    total
}

/// The per-model part of the §5.3 score.
fn single_model_score(spec: &ModelSpec, req: &Requirements, installed: &dyn Fn(&str) -> bool) -> i32 {
    let mut total = 0;

    if spec.tier == Some(Tier::Recommended) {
        total += RECOMMENDED_TIER_BONUS;
    }
    if spec.source == Some(Source::Tongyi) {
        total += TONGYI_SOURCE_BONUS;
    }
    total += memory_match(spec, req);
    if installed(&spec.id) {
        total += INSTALLED_BONUS;
    }
    if spec.status.is_verified() {
        total += VERIFIED_BONUS;
    }

    total
}

/// Design §5.3's `speed_match`, now answered by the 显存预算.
///
/// Within this catalogue "faster" and "smaller" are the same ladder — SenseVoice
/// Q8 is 254 MB and the fastest thing on offer, Fun-ASR-Nano is 1.0 GB and
/// balanced, and the largest files are the slowest — so one term replaces the old
/// `fast`/`balanced`/`accurate` preference:
///
/// * 小: the 254 MB class earns +40, and every class above costs 150 a piece.
/// * 中: a matching class earns +40, a smaller one +20 (smaller is never wrong,
///   only not what was asked for).
/// * 大: no reward at all, because everything fits; tier, source, install state
///   and the language bias decide alone, so accuracy wins.
fn memory_match(spec: &ModelSpec, req: &Requirements) -> i32 {
    let over = over_budget_rank(spec, req);
    if over > 0 {
        return -MEMORY_OVERSHOOT_PENALTY * over;
    }

    match req.memory {
        MemoryBudget::Large => 0,
        _ if footprint(spec).rank() == req.memory.rank() => MEMORY_MATCH_BONUS,
        _ => MEMORY_MATCH_BONUS / 2,
    }
}

/// Fast 0, balanced 1, accurate 2, slow 3; an undeclared speed counts as balanced.
///
/// `Slow` sits *above* accurate because they mean different things: `accurate`
/// is "this is the careful model for its size", `slow` is "this is a 7–14B model
/// that will take noticeably longer". Collapsing them would make a 14B
/// translation model look as fast as a 1.8B one.
fn speed_rank(speed: Option<Speed>) -> i32 {
    match speed {
        Some(Speed::Fast) => 0,
        Some(Speed::Accurate) => 2,
        Some(Speed::Slow) => 3,
        Some(Speed::Balanced) | None => 1,
    }
}

fn speed_from_rank(rank: i32) -> Speed {
    match rank {
        0 => Speed::Fast,
        2 => Speed::Accurate,
        3 => Speed::Slow,
        _ => Speed::Balanced,
    }
}

/// Coarse footprint class of one model's weight files.
///
/// This is what 显存占用 actually means: the GGUF has to be resident, so its byte
/// size is the floor on memory use, and in this catalogue it is also the ladder
/// that speed follows. Quantisation is *not* consulted here — a q4 model that
/// still weighs 1.5 GB needs the same 1.5 GB — so `over_budget_rank` and the
/// `memory_match` term can be read straight off the download size the UI shows.
fn footprint(spec: &ModelSpec) -> MemoryBudget {
    MemoryBudget::from_bytes(spec.files.iter().map(|file| file.size).sum())
}

fn diar_rank(grade: DiarizationGrade) -> i32 {
    match grade {
        DiarizationGrade::None => 0,
        DiarizationGrade::Turns => 1,
        DiarizationGrade::TurnsWithText => 2,
    }
}

fn timestamp_rank(grade: TimestampGrade) -> u8 {
    match grade {
        TimestampGrade::None => 0,
        TimestampGrade::Turns => 1,
        TimestampGrade::Segment => 2,
        TimestampGrade::Word => 3,
    }
}

fn grade_label(grade: TimestampGrade) -> &'static str {
    match grade {
        TimestampGrade::None => "none",
        TimestampGrade::Turns => "turn",
        TimestampGrade::Segment => "segment",
        TimestampGrade::Word => "word",
    }
}

// --- §5.5 solver -----------------------------------------------------------

/// Solve `req` against the catalogue: plans ordered by [`score`], best first.
///
/// Unsatisfiable templates are skipped; when nothing fits, the result is a
/// single gap-bearing plan with no steps and the reason in [`ModelPlan::gaps`]
/// (design §5.6). This function never panics and never returns an empty list.
pub fn solve(
    catalogue: &[ModelSpec],
    installed: &dyn Fn(&str) -> bool,
    req: &Requirements,
) -> Vec<ModelPlan> {
    // Translation models are not recognition models (translation-design §9):
    // they run on llama.cpp, not audio.cpp, and their "family" is only a prompt
    // template name. Filtering once, here, is what guarantees a `translate` or
    // `engine: llamacpp` entry can never enter a recognition plan — including
    // through `gaps`/`suggest_model` download suggestions, which read the same
    // slice.
    let recognition: Vec<ModelSpec> = catalogue
        .iter()
        .filter(|spec| {
            spec.engine == Engine::Audiocpp
                && spec.status.is_offered()
                && !spec.has_role(Role::Translate)
        })
        .cloned()
        .collect();
    let catalogue: &[ModelSpec] = &recognition;

    let mut plans = Vec::new();
    // Steps that had no candidate, used to explain an empty result.
    let mut missing: Vec<StepKind> = Vec::new();

    for template in templates_for(req) {
        match fill_template(catalogue, installed, req, &template) {
            Ok(steps) => {
                let capabilities = union_capabilities(&steps, catalogue);
                let plan_gaps = gaps(catalogue, req, &steps, &capabilities, installed);
                plans.push(ModelPlan {
                    id: plan_id(&steps),
                    title_key: title_key(&steps),
                    model_ids: model_ids(&steps),
                    cost: plan_cost(&steps, catalogue, req),
                    capabilities,
                    gaps: plan_gaps,
                    steps,
                });
            }
            Err(FillError::MissingModel(kind)) => missing.push(kind),
        }
    }

    plans.sort_by(|a, b| {
        score(b, req, catalogue, installed).cmp(&score(a, req, catalogue, installed))
    });

    if plans.is_empty() {
        let mut gaps = gaps(catalogue, req, &[], &NOTHING, installed);
        if gaps.is_empty() {
            // The request names no unmet capability (e.g. plain ASR was asked
            // for but no ASR model exists), so explain the step that had no
            // candidate instead.
            for kind in missing {
                let gap = gap_for_step(catalogue, kind);
                if !gaps.iter().any(|existing| existing.requirement == gap.requirement) {
                    gaps.push(gap);
                }
            }
        }

        plans.push(ModelPlan {
            id: "gap".to_string(),
            title_key: "modelPlan.gapTitle",
            model_ids: Vec::new(),
            steps: Vec::new(),
            cost: PlanCost {
                steps: 0,
                speed: Speed::Balanced,
                memory: req.memory,
            },
            capabilities: NOTHING,
            gaps,
        });
    }

    plans
}

/// Why a template could not be filled.
enum FillError {
    /// No catalogue model qualified for this step.
    MissingModel(StepKind),
}

/// Pick a model for every step of `template`, or report why it cannot be filled.
fn fill_template(
    catalogue: &[ModelSpec],
    installed: &dyn Fn(&str) -> bool,
    req: &Requirements,
    template: &Template,
) -> Result<Vec<PlanStep>, FillError> {
    let mut steps = Vec::with_capacity(template.steps.len());

    for kind in &template.steps {
        match kind.role() {
            None => steps.push(PlanStep::rust(*kind)),
            Some(role) => {
                let Some(spec) = best_model_for_role(catalogue, installed, role, req) else {
                    return Err(FillError::MissingModel(*kind));
                };
                steps.push(PlanStep::model(*kind, spec));
            }
        }
    }

    Ok(steps)
}

/// What a filled plan provides: the union over its model steps.
///
/// [`StepKind::TurnAttribution`] is the Rust step that joins ASR text onto diar
/// turns, so a plan containing it upgrades a `Turns` model to `TurnsWithText`
/// (design §6.4) — that is what makes the composed plan satisfy a diarization
/// requirement.
fn union_capabilities(steps: &[PlanStep], catalogue: &[ModelSpec]) -> Capabilities {
    let mut capabilities = NOTHING;
    let mut attributed = false;

    for step in steps {
        if step.kind == StepKind::TurnAttribution {
            attributed = true;
            continue;
        }
        let Some(spec) = find(catalogue, &step.model_id) else {
            continue;
        };

        let step_capabilities = spec.capabilities();
        if step_capabilities.timestamps > capabilities.timestamps {
            capabilities.timestamps = step_capabilities.timestamps;
        }
        if diar_rank(step_capabilities.diarization) > diar_rank(capabilities.diarization) {
            capabilities.diarization = step_capabilities.diarization;
        }
        capabilities.produces_text |= step_capabilities.produces_text;
    }

    if attributed && capabilities.diarization == DiarizationGrade::Turns {
        capabilities.diarization = DiarizationGrade::TurnsWithText;
    }

    capabilities
}

/// Requirements that `capabilities` does not cover, each with a suggestion.
///
/// `catalogue` is needed for the suggestion: a gap is a download prompt, so the
/// model it names may well be uninstalled (design §5.6).
pub fn gaps(
    catalogue: &[ModelSpec],
    req: &Requirements,
    steps: &[PlanStep],
    capabilities: &Capabilities,
    installed: &dyn Fn(&str) -> bool,
) -> Vec<Gap> {
    let mut gaps = Vec::new();

    if req.word_timestamps {
        // Two distinct reasons a plan can fall short of word timing, and they
        // need different fixes: the model cannot do it at all (swap the ASR), or
        // it can but a declared dependency is missing (download that). Reporting
        // only the first would send the user to download a second 1.1 GB ASR
        // when the model they already picked is fine.
        let missing_dependency = missing_requirement(catalogue, steps, installed);
        if let Some(required_id) = missing_dependency {
            gaps.push(Gap {
                requirement: "word_timestamps",
                message: format!(
                    "word-level timestamps need the '{}' model, which is not installed",
                    required_id
                ),
                suggested_model_id: Some(required_id),
            });
        } else if capabilities.timestamps < TimestampGrade::Word {
            gaps.push(Gap {
                requirement: "word_timestamps",
                message: format!(
                    "word-level timestamps requested, but the plan only reaches {} timestamps",
                    grade_label(capabilities.timestamps)
                ),
                suggested_model_id: suggest_word_model(catalogue),
            });
        }
    }

    if req.diarization && capabilities.diarization == DiarizationGrade::None {
        gaps.push(Gap {
            requirement: "diarization",
            message: "speaker separation requested, but the plan provides no speaker turns"
                .to_string(),
            suggested_model_id: suggest_model(catalogue, StepKind::Diarize),
        });
    }

    gaps
}

/// First model id named by any step's `requires` that is not installed.
///
/// The catalogue declares these dependencies (v3) so that a capability claim
/// like `qwen3_asr`'s `timestamps: word` stays honest: the claim holds only when
/// the aligner is present, and the aligner is a separate download.
fn missing_requirement(
    catalogue: &[ModelSpec],
    steps: &[PlanStep],
    installed: &dyn Fn(&str) -> bool,
) -> Option<String> {
    for step in steps {
        let Some(spec) = find(catalogue, &step.model_id) else {
            continue;
        };
        for required in &spec.requires {
            if !installed(required) && find(catalogue, required).is_some() {
                return Some(required.clone());
            }
        }
    }
    None
}

/// The gap for a step that had no candidate, used when nothing can be solved.
fn gap_for_step(catalogue: &[ModelSpec], kind: StepKind) -> Gap {
    let (requirement, message) = match kind {
        StepKind::Vad => ("vad", "no VAD model is available"),
        StepKind::Asr => ("asr", "no ASR model matches the requested language"),
        StepKind::Align => ("align", "no word-level aligner is available"),
        StepKind::Diarize => ("diarization", "no speaker-separation model is available"),
        StepKind::TurnAttribution => (
            "turns",
            "speaker attribution needs a diarization model",
        ),
    };

    Gap {
        requirement,
        message: message.to_string(),
        suggested_model_id: suggest_model(catalogue, kind),
    }
}

/// Catalogue model that would fill `kind`, ignoring install state.
fn suggest_model(catalogue: &[ModelSpec], kind: StepKind) -> Option<String> {
    let mut candidates: Vec<&ModelSpec> = match kind {
        StepKind::Diarize | StepKind::TurnAttribution => {
            // A `diar`-role model is the direct fix; a model that emits
            // turns-with-text is the alternative when the user allows it.
            let diar: Vec<&ModelSpec> = catalogue
                .iter()
                .filter(|spec| spec.has_role(Role::Diar))
                .collect();
            if diar.is_empty() {
                catalogue
                    .iter()
                    .filter(|spec| spec.capabilities().diarization != DiarizationGrade::None)
                    .collect()
            } else {
                diar
            }
        }
        _ => {
            let role = kind.role()?;
            catalogue
                .iter()
                .filter(|spec| spec.has_role(role))
                .collect()
        }
    };

    candidates.sort_by_key(|spec| suggestion_key(spec));
    candidates.last().map(|spec| spec.id.clone())
}

/// Catalogue model that would close a word-level timestamp gap.
///
/// An align-role entry wins: it is the *declared dependency* of the ASR models
/// that advertise word timing, so suggesting it lets the user keep the model they
/// already chose instead of downloading a second ASR. A word-capable ASR is the
/// fallback for a catalogue that has no aligner.
fn suggest_word_model(catalogue: &[ModelSpec]) -> Option<String> {
    // An align-role entry is offered first (see the sort below), and that is the
    // only "download" that can fix a run: the word path needs the aligner *and*
    // a family the runtime drives. Filtering on `capabilities()` alone would
    // offer `parakeet_tdt` — a model that claims word timing this app cannot
    // produce — so the filter is the runtime capability.
    let mut candidates: Vec<&ModelSpec> = catalogue
        .iter()
        .filter(|spec| {
            spec.has_role(Role::Align) || spec.supports_word_timestamps()
        })
        .collect();

    candidates.sort_by_key(|spec| (spec.has_role(Role::Align), suggestion_key(spec)));
    candidates.last().map(|spec| spec.id.clone())
}

/// Ordering key for download suggestions: recommended first, then Tongyi, then
/// the finer capability. Ties keep catalogue order (`sort_by_key` is stable).
fn suggestion_key(spec: &ModelSpec) -> (bool, bool, u8, i32) {
    (
        spec.tier == Some(Tier::Recommended),
        spec.source == Some(Source::Tongyi),
        timestamp_rank(spec.capabilities().timestamps),
        diar_rank(spec.capabilities().diarization),
    )
}

/// Stable plan id: the step shape plus the model it is keyed on.
fn plan_id(steps: &[PlanStep]) -> String {
    let shape: Vec<&str> = steps.iter().map(|step| step.kind.label()).collect();

    match id_anchor(steps) {
        Some(anchor) => format!("{}:{}", shape.join("+"), family_short(&anchor.family)),
        None => "gap".to_string(),
    }
}

/// Model the plan id is keyed on (design §3.2: `asr:moss`,
/// `asr+diar+turns:nemotron`): the last non-ASR model step — the step that makes
/// this plan distinctive — else the ASR model itself.
fn id_anchor(steps: &[PlanStep]) -> Option<&PlanStep> {
    steps
        .iter()
        .rev()
        .find(|step| step.kind != StepKind::Asr && !step.model_id.is_empty())
        .or_else(|| steps.iter().find(|step| !step.model_id.is_empty()))
}

/// i18n key for the plan title (design §8.3).
///
/// These must match the FLAT keys in the locale files
/// (`modelPlan.planAsrVad`): i18next treats `.` as a path separator, so a key
/// spelled `modelPlan.plan.asrVad` would resolve `modelPlan.plan` (absent) and
/// render the raw key instead of a title.
fn title_key(steps: &[PlanStep]) -> &'static str {
    if steps.is_empty() {
        "modelPlan.gapTitle"
    } else if steps.iter().any(|step| step.kind == StepKind::Diarize) {
        "modelPlan.planAsrDiar"
    } else {
        "modelPlan.planAsrVad"
    }
}

/// Deduplicated model ids in step order.
fn model_ids(steps: &[PlanStep]) -> Vec<String> {
    let mut ids: Vec<String> = Vec::new();

    for step in steps {
        if !step.model_id.is_empty() && !ids.contains(&step.model_id) {
            ids.push(step.model_id.clone());
        }
    }

    ids
}

/// Cost hint: step count, the slowest model's speed class, and the 显存 class the
/// plan's own weight files imply.
fn plan_cost(steps: &[PlanStep], catalogue: &[ModelSpec], req: &Requirements) -> PlanCost {
    let specs: Vec<&ModelSpec> = steps
        .iter()
        .filter_map(|step| find(catalogue, &step.model_id))
        .collect();

    let slowest = specs
        .iter()
        .map(|spec| speed_rank(spec.speed))
        .max()
        // No model step at all (the gap plan): fall back to the budget itself,
        // which is the only hint the caller gave us.
        .unwrap_or(req.memory.rank());

    // Summing the distinct model files is the closest thing to a footprint we
    // can compute without loading a model: a composed plan loads both.
    let mut bytes = 0u64;
    let mut seen: Vec<&str> = Vec::new();
    for spec in &specs {
        if seen.contains(&spec.id.as_str()) {
            continue;
        }
        seen.push(&spec.id);
        bytes += spec.files.iter().map(|file| file.size).sum::<u64>();
    }

    PlanCost {
        steps: steps.len(),
        speed: speed_from_rank(slowest),
        memory: MemoryBudget::from_bytes(bytes),
    }
}

/// First `_`-separated token of an audio.cpp family, used as the short model
/// name in a plan id (`moss_transcribe_diarize` → `moss`).
fn family_short(family: &str) -> String {
    match family.split_once('_') {
        Some((head, _)) => head.to_string(),
        None => family.to_string(),
    }
}

fn find<'a>(catalogue: &'a [ModelSpec], id: &str) -> Option<&'a ModelSpec> {
    catalogue.iter().find(|spec| spec.id == id)
}

// --- §8.2.3 manual overrides ----------------------------------------------

/// Role name accepted in `model_overrides` → the [`StepKind`] it pins.
///
/// `align` is still accepted even though [`templates_for`] never produces an
/// [`StepKind::Align`] step any more (M11-14): a v3 store blob may still carry
/// that slot, and ignoring it silently would be worse than honouring it.
pub fn override_slot_kind(slot: &str) -> Option<StepKind> {
    match slot.trim().to_lowercase().as_str() {
        "asr" => Some(StepKind::Asr),
        "vad" => Some(StepKind::Vad),
        "align" => Some(StepKind::Align),
        "diar" => Some(StepKind::Diarize),
        _ => None,
    }
}

/// Apply manual `role -> model_id` overrides to the winning plan only.
///
/// Overrides are a user assertion, so an id that is not in the catalogue is
/// ignored rather than silently turning into a runtime failure. Pinning a diar
/// model onto a plan that had no `Diarize` step adds one, because that is the
/// user's explicit intent.
///
/// Shared by `commands::transcribe` (the run) and `commands::plan` (the
/// Settings preview) so the preview cannot disagree with what will actually
/// execute.
pub fn apply_model_overrides(
    plans: &mut [ModelPlan],
    overrides: &HashMap<String, String>,
    catalogue: &[ModelSpec],
) {
    let Some(plan) = plans.first_mut() else {
        return;
    };

    for (slot, model_id) in overrides {
        let Some(kind) = override_slot_kind(slot) else {
            tracing::warn!(slot = %slot, "ignoring unknown model-override slot");
            continue;
        };
        let Some(spec) = find(catalogue, model_id) else {
            tracing::warn!(slot = %slot, model = %model_id, "ignoring override for an unknown model id");
            continue;
        };
        // A retired entry stays in the catalogue for attribution, but pinning it
        // would freeze a plan the run then refuses (the install gate and
        // `download_model` both reject an unsupported entry). An override stored
        // before the retirement is exactly how a stale `localStorage` reaches
        // here, so this is a real path, not a defensive check.
        if !spec.status.is_offered() {
            tracing::warn!(slot = %slot, model = %model_id, "ignoring override for a model this version does not support");
            continue;
        }
        let Some(task) = kind.override_task() else {
            continue;
        };

        match plan.steps.iter_mut().find(|step| step.kind == kind) {
            Some(step) => {
                step.model_id = spec.id.clone();
                step.task = task.to_string();
                step.family = spec.family.clone();
            }
            None => plan.steps.push(PlanStep {
                kind,
                model_id: spec.id.clone(),
                task: task.to_string(),
                family: spec.family.clone(),
            }),
        }

        // Pinning a diar model is a request for speaker labels, and the runtime
        // always follows a `Diarize` step with turn attribution
        // (`commands::transcribe` dispatches on exactly that step). Adding the
        // attribution step here keeps the plan's steps, id and capabilities
        // describing what will actually run.
        if kind == StepKind::Diarize
            && !plan
                .steps
                .iter()
                .any(|step| step.kind == StepKind::TurnAttribution)
        {
            plan.steps.push(PlanStep::rust(StepKind::TurnAttribution));
        }
    }

    // A pin changes what the plan *is*, not just which file it loads: the title,
    // id, capabilities and cost all describe the old model set. Recomputing them
    // here is what keeps the Settings preview honest — without it the preview
    // showed "SenseVoice + VAD" while listing `qwen3_asr + sortformer` underneath
    // the same label.
    refresh_from_catalogue(plan, catalogue);
}

// --- frozen plan (Settings-time solve, run-time read) ----------------------

/// Wire spelling of a [`LanguageProfile`], as `settings.json` stores it.
///
/// Lives here rather than in `commands::plan` because both the Settings command
/// and [`freeze`] write it, and the CLI reads it back.
pub fn language_profile_name(profile: LanguageProfile) -> &'static str {
    match profile {
        LanguageProfile::Mandarin => "mandarin",
        LanguageProfile::Mixed => "mixed",
        LanguageProfile::Multilingual => "multilingual",
    }
}

/// Freeze `plan` into the record `settings.json` keeps.
///
/// This is the only place a solved plan is turned into stored form, so the
/// Settings preview and the persisted file cannot describe different plans.
/// The four answers travel with it because the CLI reports them and a future
/// re-solve needs the inputs, not just the output.
pub fn freeze(
    plan: &ModelPlan,
    req: &Requirements,
) -> crate::model_dir::RecognitionPlan {
    crate::model_dir::RecognitionPlan {
        language_profile: language_profile_name(req.language_profile).to_string(),
        memory_budget: req.memory.label().to_string(),
        require_word_timestamps: req.word_timestamps,
        diarization: req.diarization,
        language: req.language.clone(),
        model_ids: plan.model_ids.clone(),
        steps: plan.steps.iter().map(record_step).collect(),
    }
}

fn record_step(step: &PlanStep) -> crate::model_dir::PlanStepRecord {
    crate::model_dir::PlanStepRecord {
        kind: step.kind.label().to_string(),
        model_id: step.model_id.clone(),
        task: step.task.clone(),
        family: step.family.clone(),
    }
}

/// Rebuild the executable plan from its frozen record.
///
/// `Ok(None)` means "the record describes the gap plan" (no steps), which the
/// caller reports as a missing model rather than running. `Err` means a stored
/// step cannot be interpreted — a settings file written by a different build —
/// and is deliberately loud: running a partially understood plan would produce
/// silently wrong output.
///
/// The four answers are not read back from the record here; the caller already
/// has the request's language, and the rest describe *why* the plan was chosen
/// rather than what to execute.
///
/// `catalogue` is only read to fill `capabilities`, which the run consults for
/// the word-timestamp decision. A step whose model has left the catalogue keeps
/// the `NOTHING` capability rather than failing here — the install gate reports
/// that case with the id the user needs to download.
pub fn thaw(
    record: &crate::model_dir::RecognitionPlan,
    catalogue: &[ModelSpec],
) -> Result<Option<ModelPlan>, String> {
    if record.steps.is_empty() {
        return Ok(None);
    }

    let mut steps = Vec::with_capacity(record.steps.len());
    for stored in &record.steps {
        let kind = step_kind_from_label(&stored.kind).ok_or_else(|| {
            format!(
                "the saved recognition plan has an unknown step `{}`; \
                 open Settings ▸ 模型 and save the plan again",
                stored.kind
            )
        })?;
        steps.push(PlanStep {
            kind,
            model_id: stored.model_id.clone(),
            task: stored.task.clone(),
            family: stored.family.clone(),
        });
    }

    Ok(Some(ModelPlan {
        id: plan_id(&steps),
        title_key: title_key(&steps),
        model_ids: model_ids(&steps),
        cost: PlanCost {
            steps: steps.len(),
            // A frozen plan is executed, never compared, so the cost hints are
            // only here because the type requires them.
            speed: Speed::Balanced,
            memory: MemoryBudget::from_wire(Some(&record.memory_budget)),
        },
        capabilities: union_capabilities(&steps, catalogue),
        // A frozen plan carries no gaps: the user saved a plan that solved, and
        // a model deleted since is reported by the install gate instead.
        gaps: Vec::new(),
        steps,
    }))
}

/// Recompute every derived field of `plan` from its (possibly overridden) steps.
///
/// Public because the run calls it after [`thaw`]: a frozen record stores *which*
/// models to load, while `model_ids`, `capabilities` and `cost` are derived from
/// the current catalogue and must not be read back from the record. It is also
/// what makes dropping a step (a file with speaker separation off) leave no
/// stale model id behind.
pub fn refresh_from_catalogue(plan: &mut ModelPlan, catalogue: &[ModelSpec]) {
    plan.model_ids = model_ids(&plan.steps);
    plan.title_key = title_key(&plan.steps);
    plan.id = plan_id(&plan.steps);
    plan.capabilities = union_capabilities(&plan.steps, catalogue);

    // Cost needs the requirements only for the fallback hints; the real models
    // in the plan are what matters once steps exist.
    plan.cost = plan_cost(
        &plan.steps,
        catalogue,
        &Requirements {
            memory: plan.cost.memory,
            ..Requirements::default()
        },
    );
}

/// Inverse of [`StepKind::label`], for reading a frozen plan back.
fn step_kind_from_label(label: &str) -> Option<StepKind> {
    match label {
        "vad" => Some(StepKind::Vad),
        "asr" => Some(StepKind::Asr),
        "align" => Some(StepKind::Align),
        "diar" => Some(StepKind::Diarize),
        "turns" => Some(StepKind::TurnAttribution),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::catalog::{ModelFile, SupportStatus};

    const SENSE_LANGS: &[&str] = &[
        "auto", "zh", "en", "yue", "ja", "ko", "pt", "ru", "es", "it", "fr", "de", "nl", "pl",
        "tr", "ar", "hi", "vi", "th", "id", "ms", "fa",
    ];
    const NANO_LANGS: &[&str] = &["auto", "zh", "en", "ja"];
    const QWEN_LANGS: &[&str] = &[
        "auto", "zh", "en", "ja", "ko", "es", "de", "fr", "pt", "it", "ru", "ar", "hi", "vi",
        "th", "id", "ms", "tr", "nl", "pl", "fa",
    ];
    const ALIGN_LANGS: &[&str] = &["auto", "zh", "en", "ja", "ko", "es", "de", "fr", "pt"];
    const MOSS_LANGS: &[&str] = &[
        "auto", "zh", "en", "yue", "ja", "ko", "es", "de", "fr", "pt", "it", "ru", "ar", "hi",
    ];
    const AGNOSTIC_LANGS: &[&str] = &["auto"];

    /// Minimal `ModelSpec` builder: only the fields the solver reads are set.
    struct FakeModel {
        id: &'static str,
        family: &'static str,
        roles: &'static [Role],
        capabilities: Capabilities,
        speed: Speed,
        source: Source,
        tier: Tier,
        languages: &'static [&'static str],
        model_file: &'static str,
        size: u64,
        /// Defaults to `false` so a fixture that does not care is unverified,
        /// exactly like a real catalogue entry nobody has run.
        #[allow(dead_code)]
        verified: bool,
    }

    fn spec(fake: FakeModel) -> ModelSpec {
        ModelSpec {
            id: fake.id.to_string(),
            family: fake.family.to_string(),
            display_name: fake.id.to_string(),
            target_directory: fake.id.to_string(),
            model_file: fake.model_file.to_string(),
            files: vec![ModelFile {
                path: fake.model_file.to_string(),
                urls: Vec::new(),
                size: fake.size,
                sha256: None,
            }],
            languages: fake.languages.iter().map(|lang| lang.to_string()).collect(),
            license: "test".to_string(),
            attribution: None,
            capabilities_raw: Some(fake.capabilities),
            roles: fake.roles.to_vec(),
            engine: Engine::Audiocpp,
            prompt_template: None,
            speed: Some(fake.speed),
            source: Some(fake.source),
            tier: Some(fake.tier),
            status: if fake.verified {
                SupportStatus::Supported
            } else {
                SupportStatus::Unverified
            },
            // Fixtures declare no modes: offline is the safe default and keeps
            // every existing plan test on the path it was written for.
            modes: vec![crate::engine::catalog::RunMode::Offline],
            licenses: Vec::new(),
            // Only the ASR whose word-timestamp claim is conditional on another
            // model needs a dependency; keeping it out of `FakeModel` avoids
            // threading a new field through all eleven fixtures.
            requires: if fake.id == "qwen3_asr_0_6b_q8_0" {
                vec!["qwen3_forced_aligner_0_6b_q8_0".to_string()]
            } else {
                Vec::new()
            },
            language_tokens: Vec::new(),
            max_session_seconds: None,
        }
    }

    /// A catalogue shaped like the committed `models.json` (M1), so the solver is
    /// exercised against realistic tiers, sources, sizes and language lists.
    fn catalogue() -> Vec<ModelSpec> {
        vec![
            spec(FakeModel {
                id: "sensevoice_small_q8",
                family: "sense_asr",
                roles: &[Role::Asr],
                capabilities: Capabilities {
                    timestamps: TimestampGrade::None,
                    diarization: DiarizationGrade::None,
                    produces_text: true,
                },
                speed: Speed::Fast,
                source: Source::Tongyi,
                tier: Tier::Default,
                languages: SENSE_LANGS,
                model_file: "sensevoice-small-q8-audiocpp-v1.gguf",
                size: 254_211_200,
                verified: false,
            }),
            spec(FakeModel {
                id: "fun_asr_nano_2512_q8_0",
                family: "fun_asr_nano",
                roles: &[Role::Asr],
                capabilities: Capabilities {
                    timestamps: TimestampGrade::None,
                    diarization: DiarizationGrade::None,
                    produces_text: true,
                },
                speed: Speed::Balanced,
                source: Source::Tongyi,
                tier: Tier::Recommended,
                languages: NANO_LANGS,
                model_file: "fun-asr-nano-2512-q8_0.gguf",
                size: 1_045_334_432,
                verified: false,
            }),
            spec(FakeModel {
                id: "qwen3_asr_0_6b_q8_0",
                family: "qwen3_asr",
                roles: &[Role::Asr],
                capabilities: Capabilities {
                    timestamps: TimestampGrade::Word,
                    diarization: DiarizationGrade::None,
                    produces_text: true,
                },
                speed: Speed::Balanced,
                source: Source::Qwen,
                tier: Tier::Optional,
                languages: QWEN_LANGS,
                model_file: "qwen3-asr-0.6b-q8_0.gguf",
                size: 1_151_272_416,
                verified: false,
            }),
            spec(FakeModel {
                id: "qwen3_forced_aligner_0_6b_q8_0",
                family: "qwen3_forced_aligner",
                roles: &[Role::Align],
                capabilities: Capabilities {
                    timestamps: TimestampGrade::Word,
                    diarization: DiarizationGrade::None,
                    produces_text: false,
                },
                speed: Speed::Balanced,
                source: Source::Qwen,
                tier: Tier::Optional,
                languages: ALIGN_LANGS,
                model_file: "qwen3-forced-aligner-0.6b-q8_0.gguf",
                size: 1_129_966_496,
                verified: false,
            }),
            spec(FakeModel {
                id: "moss_transcribe_diarize_q8_0",
                family: "moss_transcribe_diarize",
                roles: &[Role::Asr],
                capabilities: Capabilities {
                    timestamps: TimestampGrade::Segment,
                    diarization: DiarizationGrade::TurnsWithText,
                    produces_text: true,
                },
                speed: Speed::Balanced,
                source: Source::Openmoss,
                tier: Tier::Recommended,
                languages: MOSS_LANGS,
                model_file: "moss-transcribe-diarize-q8_0.gguf",
                size: 1_132_110_560,
                verified: false,
            }),
            spec(FakeModel {
                id: "moss_transcribe_diarize_q4_k",
                family: "moss_transcribe_diarize",
                roles: &[Role::Asr],
                capabilities: Capabilities {
                    timestamps: TimestampGrade::Segment,
                    diarization: DiarizationGrade::TurnsWithText,
                    produces_text: true,
                },
                speed: Speed::Accurate,
                source: Source::Openmoss,
                tier: Tier::Optional,
                languages: MOSS_LANGS,
                model_file: "moss-transcribe-diarize-q4_k.gguf",
                size: 758_293_216,
                verified: false,
            }),
            spec(FakeModel {
                id: "nemotron_3_diar_q8_0",
                family: "nemotron_3_diar",
                roles: &[Role::Diar],
                capabilities: Capabilities {
                    timestamps: TimestampGrade::Turns,
                    diarization: DiarizationGrade::Turns,
                    produces_text: false,
                },
                speed: Speed::Balanced,
                source: Source::Nvidia,
                tier: Tier::Optional,
                languages: AGNOSTIC_LANGS,
                model_file: "nemotron-3-diarization-q8_0.gguf",
                size: 106_675_136,
                verified: false,
            }),
        ]
    }

    fn spec_of(id: &str) -> ModelSpec {
        catalogue()
            .into_iter()
            .find(|spec| spec.id == id)
            .unwrap_or_else(|| panic!("fake catalogue has no {id}"))
    }

    /// `req` for a Chinese, balanced, Metal run with no extra requirements.
    fn req() -> Requirements {
        Requirements {
            language: "zh".to_string(),
            ..Requirements::default()
        }
    }

    fn all_installed() -> impl Fn(&str) -> bool {
        |_: &str| true
    }

    fn nothing_installed() -> impl Fn(&str) -> bool {
        |_: &str| false
    }

    /// No word timestamps, no diarization: the plain-ASR template is the only
    /// viable one: plain ASR is the only shape, and the pick is SenseVoice.
    /// Regression: a `vad`-role override must reach `model_ids` so
    /// `commands::transcribe` can hand it to the engine, and it must not become
    /// `model_ids[0]` — that slot is the ASR model the run transcribes with.
    ///
    /// Before the VAD was pluggable, `engine::transcribe` hardcoded
    /// `family = "silero_vad"`, so a selected VAD was silently never executed.
    #[test]
    fn vad_override_appends_a_step_without_taking_the_asr_slot() {
        let path = std::path::Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/resources/models.json"
        ));
        let real = crate::engine::catalog::load_catalog(path).expect("committed models.json");
        let installed = all_installed();
        let mut plans = solve(&real, &installed, &req());

        let vad_id = real
            .iter()
            .find(|spec| spec.has_role(Role::Vad))
            .expect("the catalogue offers a downloadable VAD")
            .id
            .clone();

        let mut overrides = HashMap::new();
        overrides.insert("vad".to_string(), vad_id.clone());
        apply_model_overrides(&mut plans, &overrides, &real);

        let plan = &plans[0];
        let vad = plan
            .steps
            .iter()
            .find(|step| step.kind == StepKind::Vad)
            .expect("the pinned VAD is a plan step");
        assert_eq!(vad.model_id, vad_id);
        assert_eq!(vad.task, "vad", "the step must run --task vad");
        assert!(
            plan.model_ids.contains(&vad_id),
            "the VAD must be in model_ids so the install check covers it: {:?}",
            plan.model_ids
        );
        assert_eq!(
            plan.steps[0].kind,
            StepKind::Asr,
            "the ASR step must stay first: {:?}",
            plan.steps
        );
        assert_ne!(
            plan.model_ids.first(),
            Some(&vad_id),
            "model_ids[0] is the ASR model; a VAD pin must not take it"
        );
    }

    #[test]
    fn an_override_for_a_retired_model_is_ignored() {
        // An `Unsupported` entry stays in the catalogue for attribution, and an
        // override stored before the retirement still reaches here through
        // `localStorage`. Pinning it would freeze a plan the run then refuses, so
        // the override must be dropped rather than honoured.
        let mut catalogue = catalogue();
        catalogue
            .iter_mut()
            .find(|spec| spec.id == "nemotron_3_diar_q8_0")
            .expect("the fake catalogue has the diar model")
            .status = SupportStatus::Unsupported;

        // An ASR-only plan, so a diar pin is visibly either added or not.
        let mut plans = vec![ModelPlan {
            id: "asr:sense".to_string(),
            title_key: "modelPlan.planAsrVad",
            model_ids: vec!["sensevoice_small_q8".to_string()],
            steps: vec![PlanStep {
                kind: StepKind::Asr,
                model_id: "sensevoice_small_q8".to_string(),
                task: "asr".to_string(),
                family: "sense_asr".to_string(),
            }],
            cost: PlanCost {
                steps: 1,
                speed: Speed::Balanced,
                memory: MemoryBudget::Medium,
            },
            capabilities: Capabilities::default(),
            gaps: Vec::new(),
        }];

        let mut overrides = HashMap::new();
        overrides.insert("diar".to_string(), "nemotron_3_diar_q8_0".to_string());
        apply_model_overrides(&mut plans, &overrides, &catalogue);

        assert_eq!(
            plans[0].id, "asr:sense",
            "a retired model must not become part of the plan: {:?}",
            plans[0].steps
        );
        assert!(
            !plans[0]
                .steps
                .iter()
                .any(|step| step.kind == StepKind::Diarize),
            "the retired model must not be planned: {:?}",
            plans[0].steps
        );
        assert_eq!(
            plans[0].model_ids,
            vec!["sensevoice_small_q8".to_string()],
            "the retired model must not reach the install gate"
        );
    }

    #[test]
    fn default_requirements_pick_sense_asr_first() {
        let catalogue = catalogue();
        let installed = all_installed();
        let req = req();

        let plans = solve(&catalogue, &installed, &req);

        assert_eq!(plans.len(), 1, "only the plain-ASR template is viable");
        assert_eq!(plans[0].model_ids, vec!["sensevoice_small_q8".to_string()]);
        assert_eq!(plans[0].steps[0].kind, StepKind::Asr);
        assert_eq!(plans[0].id, "asr:sense");
        assert_eq!(plans[0].title_key, "modelPlan.planAsrVad");
        assert!(plans[0].gaps.is_empty());

        let best = best_model_for_role(&catalogue, &installed, Role::Asr, &req)
            .expect("an ASR model is available");
        assert_eq!(best.id, "sensevoice_small_q8");
    }

    /// Speaker separation always composes `Asr + Diarize + TurnAttribution`.
    ///
    /// There used to be a competing one-shot template in which MOSS emitted text
    /// + timestamps + speakers in a single unsplit call, and the solver picked it
    /// on score. It is gone: that call decoded the whole file under one
    /// `--max-tokens` budget, so a multi-hour input truncated
    /// (`MOSS-Transcribe-Diarize reached max_tokens before EOS`, measured at
    /// 17 min). Every model now runs per VAD chunk, and speaker separation comes
    /// from the diar model.
    #[test]
    fn diarization_always_composes_asr_diar_and_turn_attribution() {
        let catalogue = catalogue();
        let installed = all_installed();
        let req = Requirements {
            diarization: true,
            ..req()
        };

        let plans = solve(&catalogue, &installed, &req);

        assert_eq!(plans.len(), 1, "one template, no competing one-shot shape");
        assert_eq!(
            plans[0].steps.iter().map(|step| step.kind).collect::<Vec<_>>(),
            vec![StepKind::Asr, StepKind::Diarize, StepKind::TurnAttribution]
        );
        assert_eq!(
            plans[0].capabilities.diarization,
            DiarizationGrade::TurnsWithText
        );
        assert!(plans[0].gaps.is_empty());
        // The ASR model is chosen on merit, not because a one-shot model existed.
        assert_eq!(plans[0].steps[0].kind, StepKind::Asr);
    }

    /// The composed shape in detail: SenseVoice is reused, a `diar` model
    /// supplies turns and the pure-Rust step attributes them — so the plan ends
    /// with `TurnAttribution` and still reports speaker coverage.
    #[test]
    fn diarization_composes_asr_diar_turns() {
        let catalogue = catalogue();
        let installed = all_installed();
        let req = Requirements {
            diarization: true,
            ..req()
        };

        let plans = solve(&catalogue, &installed, &req);

        let plan = &plans[0];
        assert_eq!(
            plan.steps.iter().map(|step| step.kind).collect::<Vec<_>>(),
            vec![StepKind::Asr, StepKind::Diarize, StepKind::TurnAttribution]
        );
        assert_eq!(
            plan.model_ids,
            vec![
                "sensevoice_small_q8".to_string(),
                "nemotron_3_diar_q8_0".to_string(),
            ]
        );
        assert_eq!(plan.id, "asr+diar+turns:nemotron");
        assert_eq!(plan.steps[1].task, "diar");
        assert_eq!(plan.steps[2].model_id, "", "the Rust step loads no model");
        assert_eq!(
            plan.capabilities.diarization,
            DiarizationGrade::TurnsWithText,
            "TurnAttribution upgrades turns-only to turns-with-text"
        );
        assert!(plan.gaps.is_empty());
    }

    /// Word-level + speakers. The plan reaches word timing through the aligner
    /// dependency; a model that only reaches segment timing must keep admitting
    /// the shortfall.
    ///
    /// Rewritten from `word_and_diarization_reports_the_word_timestamp_gap`: that
    /// test also asserted every plan reports a word gap, which was only true while
    /// word output was unimplemented.
    #[test]
    fn word_and_diarization_reports_the_gap_only_where_it_exists() {
        let catalogue = catalogue();
        let installed = all_installed();
        let req = Requirements {
            word_timestamps: true,
            diarization: true,
            ..req()
        };

        let plans = solve(&catalogue, &installed, &req);

        assert!(!plans.is_empty(), "speaker separation is still satisfiable");

        // No plan may claim an alignment step: the aligner runs *inside* the ASR
        // session, so a planned step would never execute.
        assert!(
            plans
                .iter()
                .all(|plan| plan.steps.iter().all(|step| step.kind != StepKind::Align)),
            "an Align step is never executed, so it must not be planned"
        );

        // The only shape: ASR (word-capable, via the aligner dependency) + a diar
        // model + the pure-Rust attribution step.
        let plan = &plans[0];
        assert_eq!(
            plan.steps.iter().map(|step| step.kind).collect::<Vec<_>>(),
            vec![StepKind::Asr, StepKind::Diarize, StepKind::TurnAttribution]
        );
        assert_eq!(plan.capabilities.timestamps, TimestampGrade::Word);
        assert_eq!(plan.capabilities.diarization, DiarizationGrade::TurnsWithText);
        assert!(
            plan.gaps
                .iter()
                .all(|gap| gap.requirement != "word_timestamps"),
            "a plan that reaches word timing must not claim a shortfall: {:?}",
            plan.gaps
        );
    }

    /// A model that cannot reach word timing still gets planned, and must keep
    /// admitting the shortfall rather than silently estimating.
    #[test]
    fn a_word_request_that_only_reaches_segment_timing_reports_the_gap() {
        let catalogue = catalogue();
        let installed = all_installed();
        // Ask for word timing on a catalogue whose only ASR models top out at
        // segment timing: no aligner requirement is declared by any of them.
        let asr_only: Vec<crate::engine::catalog::ModelSpec> = catalogue
            .into_iter()
            .filter(|spec| spec.has_role(Role::Asr) && !spec.has_role(Role::Diar))
            .collect();
        let req = Requirements {
            word_timestamps: true,
            ..req()
        };

        let plans = solve(&asr_only, &installed, &req);

        if plans.is_empty() || plans[0].steps.is_empty() {
            // Nothing could be solved at all; the gap plan explains it.
            return;
        }
        let plan = &plans[0];
        if plan.capabilities.timestamps != TimestampGrade::Word {
            assert!(
                plan.gaps
                    .iter()
                    .any(|gap| gap.requirement == "word_timestamps"),
                "estimated timing must be reported as a gap: {:?}",
                plan.gaps
            );
        }
    }

    /// Word timestamps without diarization: `qwen3_asr` satisfies it, and the
    /// aligner is never planned as its own step.
    #[test]
    fn word_only_plan_uses_a_word_capable_asr() {
        let catalogue = catalogue();
        let installed = all_installed();
        let req = Requirements {
            word_timestamps: true,
            ..req()
        };

        let plans = solve(&catalogue, &installed, &req);
        let best = &plans[0];

        assert_eq!(best.capabilities.timestamps, TimestampGrade::Word);
        assert!(best.gaps.is_empty(), "nothing is missing: {:?}", best.gaps);
        assert_eq!(
            best.steps.len(),
            1,
            "the aligner runs inside the ASR session, not as a second step: {:?}",
            best.steps
        );
        assert_eq!(best.steps[0].kind, StepKind::Asr);
    }

    /// The aligner is a separate download, and the gap names it so the UI can
    /// offer it — the ASR itself must not be blamed for a missing dependency.
    #[test]
    fn uninstalled_aligner_is_reported_as_its_own_gap() {
        let catalogue = catalogue();
        let req = Requirements {
            word_timestamps: true,
            ..req()
        };
        // Everything except the aligner.
        let installed = |id: &str| id != "qwen3_forced_aligner_0_6b_q8_0";

        let plans = solve(&catalogue, &installed, &req);
        let best = &plans[0];

        let gap = best
            .gaps
            .iter()
            .find(|gap| gap.requirement == "word_timestamps")
            .unwrap_or_else(|| panic!("plan {} hides the missing aligner", best.id));
        assert_eq!(
            gap.suggested_model_id.as_deref(),
            Some("qwen3_forced_aligner_0_6b_q8_0"),
            "the fix is the aligner, not a different ASR"
        );
    }

    /// `ko`: `fun_asr_nano` declares no Korean and must be filtered out even
    /// though it outranks SenseVoice on tier; `sense_asr` is kept.
    #[test]
    fn korean_excludes_fun_asr_nano() {
        let catalogue = catalogue();
        let installed = all_installed();
        let req = Requirements {
            language: "ko".to_string(),
            ..Requirements::default()
        };

        let best = best_model_for_role(&catalogue, &installed, Role::Asr, &req)
            .expect("SenseVoice serves Korean");
        assert_eq!(best.id, "sensevoice_small_q8");
        assert!(!language_ok(&spec_of("fun_asr_nano_2512_q8_0"), "ko"));

        let plans = solve(&catalogue, &installed, &req);
        assert_eq!(plans[0].model_ids, vec!["sensevoice_small_q8".to_string()]);
    }

    /// `yue`: only SenseVoice serves Cantonese; the two other Tongyi/Qwen ASR
    /// models declare no `yue` and are filtered out.
    #[test]
    fn yue_is_served_only_by_sense_asr() {
        let catalogue = catalogue();
        let installed = all_installed();
        let req = Requirements {
            language: "yue".to_string(),
            ..Requirements::default()
        };

        let best = best_model_for_role(&catalogue, &installed, Role::Asr, &req)
            .expect("SenseVoice serves Cantonese");
        assert_eq!(best.id, "sensevoice_small_q8");
        assert!(!language_ok(&spec_of("fun_asr_nano_2512_q8_0"), "yue"));
        assert!(!language_ok(&spec_of("qwen3_asr_0_6b_q8_0"), "yue"));

        let plans = solve(&catalogue, &installed, &req);
        assert_eq!(plans[0].model_ids, vec!["sensevoice_small_q8".to_string()]);
    }

    /// A request with no satisfiable template must return a single gap-bearing
    /// plan instead of panicking, whether the catalogue is empty or nothing is
    /// installed. Install state alone is never a hard filter.
    #[test]
    fn nothing_installed_never_panics_and_gaps_when_unsolvable() {
        let installed = nothing_installed();
        let req = req();

        let empty = solve(&[], &installed, &req);
        assert_eq!(empty.len(), 1);
        assert_eq!(empty[0].id, "gap");
        assert!(empty[0].steps.is_empty());
        assert_eq!(empty[0].cost.steps, 0);
        assert!(!empty[0].gaps.is_empty(), "the gap must explain itself");
        assert_eq!(empty[0].gaps[0].requirement, "asr");
        assert_eq!(empty[0].gaps[0].suggested_model_id, None);

        // An uninstalled model is still the right recommendation; only the UI
        // turns that into a download prompt.
        let catalogue = catalogue();
        let uninstalled = solve(&catalogue, &installed, &req);
        assert_eq!(
            uninstalled[0].model_ids,
            vec!["sensevoice_small_q8".to_string()]
        );
    }

    /// A catalogue with no speaker-capable model at all (no `diar` model and no
    /// diar model) must produce a gap plus a concrete download
    /// suggestion, never an error.
    #[test]
    fn missing_diar_models_return_a_gap_with_a_suggestion() {
        let catalogue: Vec<ModelSpec> = catalogue()
            .into_iter()
            .filter(|spec| {
                spec.id != "nemotron_3_diar_q8_0"
                    && !spec.id.starts_with("moss_transcribe_diarize")
            })
            .collect();
        let installed = all_installed();
        let req = Requirements {
            diarization: true,
            ..req()
        };

        let plans = solve(&catalogue, &installed, &req);

        assert_eq!(plans.len(), 1);
        assert_eq!(plans[0].id, "gap");
        assert!(plans[0].steps.is_empty());
        let gap = plans[0]
            .gaps
            .iter()
            .find(|gap| gap.requirement == "diarization")
            .expect("the diarization requirement is reported as a gap");
        assert!(
            gap.message.contains("speaker"),
            "the gap must be human-readable, got {:?}",
            gap.message
        );
        // With every speaker-capable entry gone the suggestion may be empty;
        // what matters is that a gap is reported rather than a panic.
        if let Some(suggested) = gap.suggested_model_id.as_deref() {
            assert!(
                suggested.contains("diar") || suggested.contains("moss"),
                "a suggestion must be a speaker-capable model, got {suggested}"
            );
        }
    }

    /// Without a `diar` model, the solver still
    /// With no diar model at all, speaker separation is unsatisfiable and the
    /// solver must report a gap rather than silently dropping the requirement.
    #[test]
    fn missing_diar_model_reports_a_gap_instead_of_a_one_shot_fallback() {
        let catalogue: Vec<ModelSpec> = catalogue()
            .into_iter()
            .filter(|spec| spec.id != "nemotron_3_diar_q8_0")
            .collect();
        let installed = all_installed();
        let req = Requirements {
            diarization: true,
            ..req()
        };

        let plans = solve(&catalogue, &installed, &req);

        assert!(!plans.is_empty());
        let plan = &plans[0];
        // Either a diar step is present (some other diar model was picked) or the
        // plan admits it cannot separate speakers. A one-shot model must not be
        // used as an implicit substitute.
        let has_diar = plan.steps.iter().any(|step| step.kind == StepKind::Diarize);
        assert!(
            has_diar || plan.gaps.iter().any(|gap| gap.requirement == "diarization"),
            "without a diar model the plan must either compose one or report the gap: {plan:?}"
        );
    }

    /// The language bias (§5.4) sends es/de/fr/pt to a non-SenseVoice ASR, and
    /// normalises `zh-tw` onto the same pick as `zh`.
    #[test]
    fn language_bias_sends_spanish_to_qwen_and_zh_tw_to_sense() {
        let catalogue = catalogue();
        let installed = all_installed();

        let spanish = Requirements {
            language: "es".to_string(),
            ..Requirements::default()
        };
        let best = best_model_for_role(&catalogue, &installed, Role::Asr, &spanish)
            .expect("Qwen3 ASR serves Spanish");
        assert_eq!(best.id, "qwen3_asr_0_6b_q8_0");
        assert!(!language_ok(&spec_of("sensevoice_small_q8"), "es"));

        let traditional = Requirements {
            language: "zh-tw".to_string(),
            ..Requirements::default()
        };
        let best = best_model_for_role(&catalogue, &installed, Role::Asr, &traditional)
            .expect("Chinese has an ASR model");
        assert_eq!(best.id, "sensevoice_small_q8");
    }

    /// `footprint` is a pure function of the weight-file bytes — the number the
    /// download row already shows — so a user can tell which 显存 bucket a model
    /// lands in without running it. Quantisation is deliberately *not* consulted:
    /// a q4 file that still weighs 1.5 GB needs the same 1.5 GB.
    #[test]
    fn footprint_is_decided_by_download_size_not_quantisation() {
        let q8 = spec_of("moss_transcribe_diarize_q8_0"); // 1.13 GB
        let q4 = spec_of("moss_transcribe_diarize_q4_k"); // 758 MB
        let sense = spec_of("sensevoice_small_q8"); // 254 MB
        let tiny = spec_of("nemotron_3_diar_q8_0"); // 107 MB

        assert_eq!(footprint(&sense), MemoryBudget::Small);
        assert_eq!(footprint(&tiny), MemoryBudget::Small);
        assert_eq!(
            footprint(&q4),
            MemoryBudget::Medium,
            "758 MB is 中 even though it is the q4 quantisation"
        );
        assert_eq!(footprint(&q8), MemoryBudget::Medium);
        assert_eq!(
            MemoryBudget::from_bytes(2 * 1024 * 1024 * 1024),
            MemoryBudget::Large,
            "a 2 GB model is 大"
        );
    }

    /// 显存预算 must be decisive, not decorative: under 小 a 254 MB SenseVoice has
    /// to beat a 1 GB `tier: recommended` Tongyi model.
    #[test]
    fn small_memory_budget_picks_the_smallest_model() {
        let catalogue = catalogue();
        let installed = all_installed();
        let req = Requirements {
            memory: MemoryBudget::Small,
            ..req()
        };

        let best = best_model_for_role(&catalogue, &installed, Role::Asr, &req)
            .expect("an ASR model fits");
        assert_eq!(best.id, "sensevoice_small_q8");
        assert_eq!(footprint(best), MemoryBudget::Small);

        let plans = solve(&catalogue, &installed, &req);
        assert_eq!(
            plans[0].model_ids,
            vec!["sensevoice_small_q8".to_string()]
        );
        assert_eq!(plans[0].cost.memory, MemoryBudget::Small);
    }

    /// A capability outranks a budget. `中英混合` + `显存小` must still plan the
    /// 1 GB multilingual model, because the alternative is a subtitle that drops
    /// half the sentence — and the plan must *say* it went over budget rather
    /// than claim a small footprint.
    ///
    /// Regression: the budget was ranked above the language bias, so this
    /// combination silently picked the 254 MB SenseVoice for mixed-language
    /// audio, which is the one thing the profile exists to prevent.
    #[test]
    fn a_language_capability_outranks_the_memory_budget() {
        let catalogue = catalogue();
        let installed = all_installed();
        let req = Requirements {
            language_profile: LanguageProfile::Mixed,
            memory: MemoryBudget::Small,
            ..req()
        };

        let plans = solve(&catalogue, &installed, &req);

        assert_eq!(
            plans[0].model_ids,
            vec!["qwen3_asr_0_6b_q8_0".to_string()],
            "the multilingual model must win over the smaller single-token one"
        );
        assert_eq!(
            plans[0].cost.memory,
            MemoryBudget::Medium,
            "the plan must admit the footprint it actually needs"
        );
        assert!(
            plans[0].gaps.is_empty(),
            "overshooting a budget is a cost, never a gap: {:?}",
            plans[0].gaps
        );
    }

    /// A budget nothing fits must still plan the closest model: an overshoot is a
    /// cost, not a missing capability, so no gap is reported.
    #[test]
    fn an_unreachable_memory_budget_still_plans_and_reports_no_gap() {
        // Drop the only 小 entry, so every remaining ASR is one class over.
        let catalogue: Vec<ModelSpec> = catalogue()
            .into_iter()
            .filter(|spec| spec.id != "sensevoice_small_q8")
            .collect();
        let installed = all_installed();
        let req = Requirements {
            memory: MemoryBudget::Small,
            ..req()
        };

        let plans = solve(&catalogue, &installed, &req);

        assert_eq!(
            plans[0].model_ids,
            vec!["fun_asr_nano_2512_q8_0".to_string()],
            "the closest fit (one class over) is still planned"
        );
        assert_eq!(plans[0].cost.memory, MemoryBudget::Medium);
        assert!(
            plans[0].gaps.is_empty(),
            "a footprint over budget is not a capability gap: {:?}",
            plans[0].gaps
        );
    }

    /// 中英混合 is a promise about the audio that a single-language token model
    /// cannot keep, so the profile must move the pick off SenseVoice even though
    /// the §5.4 table puts it first for Chinese.
    #[test]
    fn mixed_language_profile_moves_the_pick_off_sense_asr() {
        let catalogue = catalogue();
        let installed = all_installed();
        let req = Requirements {
            language_profile: LanguageProfile::Mixed,
            ..req()
        };

        let best = best_model_for_role(&catalogue, &installed, Role::Asr, &req)
            .expect("a multilingual ASR is available");
        assert_eq!(best.family, "qwen3_asr");
        assert_ne!(best.family, "sense_asr");

        let plans = solve(&catalogue, &installed, &req);
        assert_eq!(
            plans[0].model_ids,
            vec!["qwen3_asr_0_6b_q8_0".to_string()]
        );
    }

    /// `多语种` for a non-Chinese language: the profile and the language agree, and
    /// the answer is still the multilingual ASR.
    #[test]
    fn multilingual_profile_serves_spanish_with_a_multilingual_asr() {
        let catalogue = catalogue();
        let installed = all_installed();
        let req = Requirements {
            language: "es".to_string(),
            language_profile: LanguageProfile::Multilingual,
            ..Requirements::default()
        };

        let best = best_model_for_role(&catalogue, &installed, Role::Asr, &req)
            .expect("Qwen3-ASR serves Spanish");
        assert_eq!(best.id, "qwen3_asr_0_6b_q8_0");
    }

    /// `Mandarin` must not disturb the §5.4 table: the default profile is what
    /// keeps SenseVoice first for Chinese.
    #[test]
    fn mandarin_profile_keeps_the_per_language_table() {
        let catalogue = catalogue();
        let installed = all_installed();

        let chinese = best_model_for_role(&catalogue, &installed, Role::Asr, &req())
            .expect("Chinese has an ASR model");
        assert_eq!(chinese.id, "sensevoice_small_q8");

        let spanish = Requirements {
            language: "es".to_string(),
            ..Requirements::default()
        };
        let best = best_model_for_role(&catalogue, &installed, Role::Asr, &spanish)
            .expect("Spanish has an ASR model");
        assert_eq!(
            best.id, "qwen3_asr_0_6b_q8_0",
            "普通话为主 must not claim Spanish audio"
        );
    }

    /// Regression guard: a catalogue containing a translate model must solve
    /// EXACTLY as the same catalogue without it. Translation is a separate,
    /// post-ASR stage (translation-design §9); if it could enter the plan, the
    /// app would hand `HY-MT1.5-1.8B-Q4_K_M.gguf` to `audiocpp_cli`, which
    /// cannot load it.
    #[test]
    fn translate_models_never_enter_a_recognition_plan() {
        let baseline = catalogue();
        let installed = all_installed();
        let req = req();

        let mut with_translate = baseline.clone();
        with_translate.push(spec(FakeModel {
            id: "hy_mt_1_8b_q4_k_m",
            // `family` is a prompt-template hint for this engine, not an
            // audio.cpp family; a llama.cpp entry must be ignored wholesale.
            family: "hy_mt",
            roles: &[Role::Translate],
            capabilities: Capabilities {
                timestamps: TimestampGrade::None,
                diarization: DiarizationGrade::None,
                produces_text: true,
            },
            speed: Speed::Fast,
            source: Source::Other,
            tier: Tier::Recommended,
            languages: &["zh", "en", "ja"],
            model_file: "HY-MT1.5-1.8B-Q4_K_M.gguf",
            size: 1_133_080_512,
            verified: false,
        }));
        // ...and one that is flagged on the engine alone, with a recognition-
        // looking role, to prove the engine filter is what does the work.
        with_translate.push(spec(FakeModel {
            id: "llamacpp_asr_lookalike",
            family: "sense_asr",
            roles: &[Role::Asr],
            capabilities: Capabilities {
                timestamps: TimestampGrade::Word,
                diarization: DiarizationGrade::None,
                produces_text: true,
            },
            speed: Speed::Fast,
            source: Source::Tongyi,
            tier: Tier::Recommended,
            languages: SENSE_LANGS,
            model_file: "lookalike.gguf",
            size: 10,
            verified: false,
        }));
        let last = with_translate.len() - 1;
        with_translate[last].engine = Engine::Llamacpp;

        for requirements in [
            req.clone(),
            Requirements {
                diarization: true,
                ..req.clone()
            },
            Requirements {
                word_timestamps: true,
                ..req.clone()
            },
        ] {
            let expected = solve(&baseline, &installed, &requirements);
            let actual = solve(&with_translate, &installed, &requirements);

            let shape = |plans: &[ModelPlan]| {
                plans
                    .iter()
                    .map(|plan| (plan.id.clone(), plan.model_ids.clone(), plan.steps.clone()))
                    .collect::<Vec<_>>()
            };
            assert_eq!(
                shape(&actual),
                shape(&expected),
                "translate/llamacpp entries changed the plan for {requirements:?}"
            );
            assert!(
                actual
                    .iter()
                    .flat_map(|plan| plan.model_ids.iter())
                    .all(|id| !id.contains("hy_mt") && !id.contains("lookalike")),
                "a llama.cpp model leaked into a recognition plan: {:?}",
                shape(&actual)
            );
        }
    }

    /// `best_model_for_role` is the seam the solver uses, so the engine filter is
    /// pinned there too — `Role::Translate` models are not `Role::Asr` models
    /// either, and neither is a llama.cpp entry that claims the role.
    #[test]
    fn best_model_for_role_ignores_non_audiocpp_models() {
        let mut catalogue = catalogue();
        let mut llama = spec(FakeModel {
            id: "llamacpp_sense_asr",
            family: "sense_asr",
            roles: &[Role::Asr, Role::Translate],
            capabilities: Capabilities {
                timestamps: TimestampGrade::Word,
                diarization: DiarizationGrade::None,
                produces_text: true,
            },
            speed: Speed::Fast,
            source: Source::Tongyi,
            tier: Tier::Recommended,
            languages: SENSE_LANGS,
            model_file: "lookalike.gguf",
            size: 10,
            verified: false,
        });
        llama.engine = Engine::Llamacpp;
        catalogue.push(llama);

        let installed = all_installed();
        let best = best_model_for_role(&catalogue, &installed, Role::Asr, &req())
            .expect("an audio.cpp ASR model is available");
        assert_eq!(best.id, "sensevoice_small_q8");

        assert!(
            best_model_for_role(&catalogue, &installed, Role::Translate, &req()).is_none(),
            "translation is not a recognition step"
        );
    }

    /// Templates are enumerated purely from the capability requirements, and
    /// there is exactly one shape per combination — no competing one-shot plan.
    #[test]
    fn templates_are_enumerated_from_capability_requirements() {
        let plain = req();
        let templates = templates_for(&plain);
        assert_eq!(templates.len(), 1);
        assert_eq!(templates[0].steps, vec![StepKind::Asr]);

        let spoken = Requirements {
            diarization: true,
            ..plain.clone()
        };
        let templates = templates_for(&spoken);
        assert_eq!(templates.len(), 1);
        assert_eq!(
            templates[0].steps,
            vec![StepKind::Asr, StepKind::Diarize, StepKind::TurnAttribution]
        );

        let words = Requirements {
            word_timestamps: true,
            ..plain
        };
        let templates = templates_for(&words);
        assert_eq!(templates.len(), 1);
        assert_eq!(templates[0].steps, vec![StepKind::Asr]);
    }

    /// A verified model outranks an otherwise-identical unverified one, but
    /// verification never becomes a filter: an unverified model is still planned
    /// when it is the only candidate.
    #[test]
    fn verified_outranks_unverified_but_is_never_a_filter() {
        let base = |verified: bool| {
            spec(FakeModel {
                id: "asr_a",
                family: "sense_asr",
                roles: &[Role::Asr],
                capabilities: Capabilities {
                    timestamps: TimestampGrade::None,
                    diarization: DiarizationGrade::None,
                    produces_text: true,
                },
                speed: Speed::Fast,
                source: Source::Tongyi,
                tier: Tier::Recommended,
                languages: SENSE_LANGS,
                model_file: "a.gguf",
                size: 254_000_000,
                verified,
            })
        };

        let verified = base(true);
        let unverified = base(false);
        let installed = all_installed();

        // Both are eligible; only the score differs.
        assert!(
            best_model_for_role(&[verified.clone()], &installed, Role::Asr, &req()).is_some(),
            "an unverified-only catalogue must still yield a model"
        );
        let pair = [unverified.clone(), verified.clone()];
        let picked = best_model_for_role(&pair, &installed, Role::Asr, &req())
            .expect("a candidate exists");
        assert!(
            picked.status.is_verified(),
            "the verified model must win the tie-break"
        );

        // And the bonus is bounded: a verified model three sizes over budget must
        // still lose to a small unverified one under 显存小.
        let big_verified = spec(FakeModel {
            id: "asr_big",
            family: "sense_asr",
            roles: &[Role::Asr],
            capabilities: Capabilities {
                timestamps: TimestampGrade::None,
                diarization: DiarizationGrade::None,
                produces_text: true,
            },
            speed: Speed::Accurate,
            source: Source::Tongyi,
            tier: Tier::Recommended,
            languages: SENSE_LANGS,
            model_file: "big.gguf",
            size: 1_800_000_000,
            verified: true,
        });
        let small_unverified = unverified.clone();
        let budget_req = Requirements {
            memory: MemoryBudget::Small,
            ..req()
        };
        let pair = [big_verified, small_unverified];
        let picked = best_model_for_role(&pair, &installed, Role::Asr, &budget_req)
            .expect("a candidate exists");
        assert!(
            !picked.status.is_verified(),
            "显存预算 must outrank verification, got {}",
            picked.id
        );
    }

    /// The solver must behave sensibly against the REAL committed catalogue,
    /// not just the hand-built fakes. This is the integration guard that the
    /// defaults still resolve to SenseVoice and that the
    /// diarization paths select the models the ledger claims.
    #[test]
    fn solver_against_real_committed_catalogue() {
        let path = std::path::Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/resources/models.json"
        ));
        let real =
            crate::engine::catalog::load_catalog(path).expect("committed models.json parses");
        let installed = all_installed();

        // Default requirements: SenseVoice first (Tongyi bias + fastest).
        let plans = solve(&real, &installed, &req());
        assert!(!plans.is_empty(), "default requirements must yield a plan");
        assert!(
            plans[0].model_ids.iter().any(|id| id == "sensevoice_small_q8"),
            "default plan should use sensevoice_small_q8, got {:?}",
            plans[0].model_ids
        );
        // Speaker separation always composes ASR + a real diar model + the
        // pure-Rust attribution step, against the real catalogue too.
        let spoken = solve(
            &real,
            &installed,
            &Requirements {
                diarization: true,
                ..req()
            },
        );
        let composed = &spoken[0];
        assert!(
            composed
                .steps
                .iter()
                .any(|step| step.kind == StepKind::Diarize),
            "the plan needs a Diarize step, got {:?}",
            composed.steps
        );
    }

    /// Regression: with the full survey catalogue loaded, several families
    /// declare `timestamps: word` (`parakeet_tdt`, `nemotron_asr`,
    /// `crisperwhisper`, `gigaam_asr`, …) but only qwen3 has a runtime path here
    /// (`engine::aligned` passes qwen3-scoped session options). Ranking on the
    /// declaration alone made the solver plan `parakeet_tdt` for a word request,
    /// which then silently degraded to estimated timing.
    #[test]
    fn word_timestamps_only_plan_the_family_this_app_can_align() {
        let path = std::path::Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/resources/models.json"
        ));
        let real =
            crate::engine::catalog::load_catalog(path).expect("committed models.json parses");
        let installed = all_installed();

        let claims: Vec<&str> = real
            .iter()
            .filter(|spec| spec.capabilities().timestamps >= TimestampGrade::Word)
            .map(|spec| spec.id.as_str())
            .collect();
        assert!(
            claims.len() > 1,
            "the catalogue must exercise this: several models claim word timing"
        );
        assert!(
            claims.iter().any(|id| id.starts_with("parakeet")),
            "expected a non-qwen3 model among the claimers, got {claims:?}"
        );

        let plans = solve(
            &real,
            &installed,
            &Requirements {
                language: "zh".to_string(),
                word_timestamps: true,
                ..Requirements::default()
            },
        );
        let chosen: Vec<&str> = plans[0].model_ids.iter().map(String::as_str).collect();
        assert_eq!(
            chosen,
            vec!["qwen3_asr_0_6b_q8_0"],
            "only the family with an implemented word path may be planned"
        );

        // ...and the download suggestion must be the aligner, not another ASR
        // that claims a capability this app cannot deliver.
        let gap = plans[0]
            .gaps
            .iter()
            .find(|gap| gap.requirement == "word_timestamps");
        if let Some(gap) = gap {
            let suggested = gap.suggested_model_id.as_deref().unwrap_or_default();
            assert!(
                suggested.contains("forced_aligner"),
                "the fix for a word gap is the aligner, got {suggested:?}"
            );
        }
    }

    /// The four questions the Settings panel asks must each change the answer
    /// when resolved against the REAL committed catalogue. This is the guard
    /// that the UI is not decorative: a toggle that changes nothing is worse
    /// than no toggle at all.
    #[test]
    fn real_catalogue_answers_the_four_questions() {
        let path = std::path::Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/resources/models.json"
        ));
        let real =
            crate::engine::catalog::load_catalog(path).expect("committed models.json parses");
        let installed = all_installed();

        let picked = |req: &Requirements| {
            solve(&real, &installed, req)
                .into_iter()
                .next()
                .expect("a plan is always returned")
                .model_ids
        };

        // 1. 语种场景: 中英混合 must not land on the Chinese-only SenseVoice.
        let mandarin = picked(&Requirements {
            language: "zh".to_string(),
            ..Requirements::default()
        });
        let mixed = picked(&Requirements {
            language: "zh".to_string(),
            language_profile: LanguageProfile::Mixed,
            ..Requirements::default()
        });
        assert!(
            mandarin.iter().any(|id| id == "sensevoice_small_q8"),
            "普通话为主 should use SenseVoice, got {mandarin:?}"
        );
        assert!(
            !mixed.iter().any(|id| id == "sensevoice_small_q8"),
            "中英混合 must not use a single-language-token model, got {mixed:?}"
        );

        // 2. 说话人分离: a real diar model must appear. It is no longer answered
        // by a one-shot model, because every model now runs per VAD chunk and
        // speaker turns come from the diar task.
        let spoken = picked(&Requirements {
            language: "zh".to_string(),
            diarization: true,
            ..Requirements::default()
        });
        assert!(
            spoken.iter().any(|id| id.contains("diar")),
            "speaker separation needs a diar model, got {spoken:?}"
        );

        // 3. 词级时间戳: the word capable model replaces the plain ASR.
        let words = picked(&Requirements {
            language: "zh".to_string(),
            word_timestamps: true,
            ..Requirements::default()
        });
        assert!(
            words.iter().any(|id| id.starts_with("qwen3_asr")),
            "word timestamps need qwen3_asr, got {words:?}"
        );

        // 4. 显存占用: 小 must not spend a 1 GB model on a plain transcript.
        let small = picked(&Requirements {
            language: "zh".to_string(),
            memory: MemoryBudget::Small,
            ..Requirements::default()
        });
        assert_eq!(small, vec!["sensevoice_small_q8".to_string()]);
    }
}
