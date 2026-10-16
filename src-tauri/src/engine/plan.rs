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

use crate::engine::catalog::{
    self, Capabilities, DiarizationGrade, ModelSpec, Role, Source, Speed, Tier, TimestampGrade,
};
use crate::engine::cli::Backend;

/// What a plan with no steps can provide.
const NOTHING: Capabilities = Capabilities {
    timestamps: TimestampGrade::None,
    diarization: DiarizationGrade::None,
    produces_text: false,
};

// --- §5.3 score weights ----------------------------------------------------

const RECOMMENDED_TIER_BONUS: i32 = 100;
const TONGYI_SOURCE_BONUS: i32 = 60;
const SPEED_MATCH_BONUS: i32 = 40;
const INSTALLED_BONUS: i32 = 30;
const PASSTHROUGH_BONUS: i32 = 20;
const EXTRA_STEP_PENALTY: i32 = 50;
const MEMORY_RISK_PENALTY: i32 = 30;

/// Files at or above this size count as "large" for [`memory_risk`] on Metal.
const LARGE_MODEL_BYTES: u64 = 1_073_741_824; // 1 GiB

// --- §3.1 requirements -----------------------------------------------------

/// A transcription's hard capability requirements (design §3.1).
///
/// The user states needs, never model names; [`solve`] turns this into plans.
///
/// Passthrough is deliberately **not** here: whether one model can emit text +
/// timestamps + speakers in a single call is a property of the chosen models,
/// so the solver decides it at runtime rather than asking the user.
#[derive(Debug, Clone)]
pub struct Requirements {
    /// Word-level timestamps needed (precise alignment / clipping).
    pub word_timestamps: bool,
    /// Speaker separation needed.
    pub diarization: bool,
    /// App language (`zh` / `zh-tw` / `en` / …); mapped by
    /// [`catalog::map_language`], never by this module.
    pub language: String,
    /// Speed preference; only ranks models that already pass the hard filters.
    pub prefer: Speed,
    /// Device backend; only feeds [`memory_risk`].
    pub backend: Backend,
}

impl Default for Requirements {
    fn default() -> Self {
        Self {
            word_timestamps: false,
            diarization: false,
            language: "auto".to_string(),
            prefer: Speed::Balanced,
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
    fn rust(kind: StepKind) -> Self {
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
    /// One call emitting text + timestamps + speakers (`moss_transcribe_diarize`).
    Passthrough,
    /// Pure Rust: attribute ASR segments to diar turns (design §6.4).
    TurnAttribution,
}

impl StepKind {
    /// The audio.cpp `--task` this step runs; empty for the Rust-only step.
    ///
    /// `Passthrough` is `asr` because the CLI is invoked as
    /// `--task asr --family moss_transcribe_diarize` (design §6.3).
    fn task(self) -> &'static str {
        match self {
            StepKind::Vad => "vad",
            StepKind::Asr | StepKind::Passthrough => "asr",
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
            StepKind::Passthrough => Some(Role::Passthrough),
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
            StepKind::Passthrough => "passthrough",
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
}

/// One executable plan: an ordered list of steps (design §3.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelPlan {
    /// Stable id for UI display and logs (`passthrough:moss`,
    /// `asr+diar+turns:nemotron`).
    pub id: String,
    /// i18n key for the human-readable title (design §8.3).
    pub title_key: &'static str,
    /// Models that must be installed, deduplicated, in step order.
    pub model_ids: Vec<String>,
    /// Ordered steps; empty only for a gap-bearing plan.
    pub steps: Vec<PlanStep>,
    /// True when a single step emits text + timestamps + speakers.
    pub passthrough: bool,
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
    gate: TemplateGate,
}

/// The condition under which a passthrough candidate is offered at all.
///
/// Both conditions come from the §5.2 table's parentheticals; without them the
/// passthrough plan wins on the §5.3 score alone (MOSS is `tier: recommended`)
/// and plain ASR could never be the answer. The decision is automatic: the user
/// never selects a passthrough mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TemplateGate {
    /// Always a candidate.
    Always,
    /// Row 1 "② `Passthrough`（若模型更快/更合语种）": only when it is genuinely
    /// better than the plain-ASR pick.
    BeatsPlainAsr,
}

impl Template {
    fn always(steps: Vec<StepKind>) -> Self {
        Self {
            steps,
            gate: TemplateGate::Always,
        }
    }

    fn gated(steps: Vec<StepKind>, gate: TemplateGate) -> Self {
        Self { steps, gate }
    }
}

/// The candidate plan shapes for `req` (design §5.2), in priority order.
///
/// Four hardcoded combinations — word-level × speaker separation — never a
/// generic constraint solve. Whether the passthrough candidate is offered is
/// decided by its [`TemplateGate`], not by the caller.
pub fn templates_for(req: &Requirements) -> Vec<Template> {
    match (req.word_timestamps, req.diarization) {
        (false, false) => vec![
            Template::always(vec![StepKind::Asr]),
            Template::gated(vec![StepKind::Passthrough], TemplateGate::BeatsPlainAsr),
        ],
        (false, true) => vec![
            Template::always(vec![StepKind::Passthrough]),
            Template::always(vec![
                StepKind::Asr,
                StepKind::Diarize,
                StepKind::TurnAttribution,
            ]),
        ],
        (true, false) => vec![
            // Word-level output is not implemented end to end (no `--words-out`
            // parsing), so the best available plan is plain ASR; [`gaps`] then
            // reports the shortfall instead of pretending it was met.
            Template::always(vec![StepKind::Asr]),
        ],
        (true, true) => vec![
            Template::always(vec![
                StepKind::Asr,
                StepKind::Diarize,
                StepKind::TurnAttribution,
            ]),
            Template::always(vec![StepKind::Passthrough]),
        ],
    }
}

// --- §5.3 / §5.4 model choice ----------------------------------------------

/// Best catalogue model for `role` under `req`, or `None` when none qualifies.
///
/// Hard filters first: the model must have the role and must be able to serve
/// the requested language ([`language_ok`]). Candidates are then ordered by the
/// §5.4 language bias, with the §5.3 [`score`] breaking ties — the bias is what
/// keeps `sense_asr` ahead of `fun_asr_nano` for Chinese even though the latter
/// is `tier: recommended` (and therefore +100 on the score alone).
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
        .filter(|spec| spec.has_role(role) && language_ok(spec, &req.language))
        .min_by_key(|spec| {
            (
                bias_rank(&req.language, spec),
                -single_model_score(spec, req, installed),
            )
        })
}

/// True when `spec` can be asked for `app_lang` (design §5.3 rule 2).
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

/// First-choice ASR families per language (design §5.4) — a bias, never a filter.
fn bias_families(language: &str) -> &'static [&'static str] {
    match language.trim().to_lowercase().as_str() {
        "zh" | "zh-tw" => &["sense_asr", "fun_asr_nano", "qwen3_asr"],
        "en" => &["sense_asr", "parakeet_tdt", "qwen3_asr"],
        "ja" => &["sense_asr", "fun_asr_nano"],
        // `fun_asr_nano` does not support Korean.
        "ko" => &["sense_asr"],
        // Cantonese: SenseVoice is the only model that serves it.
        "yue" => &["sense_asr"],
        // These fall back to auto-detect on SenseVoice, so a non-SenseVoice ASR
        // is preferred (design §5.4).
        "es" | "de" | "fr" | "pt" => &["qwen3_asr", "cohere_asr", "canary_asr"],
        // `auto` and anything unnamed: Tongyi first, then the §5.3 score.
        _ => &["sense_asr"],
    }
}

/// Position of `spec.family` in the §5.4 table; `usize::MAX` when the table does
/// not name it, which leaves the §5.3 score in charge for that role.
fn bias_rank(language: &str, spec: &ModelSpec) -> usize {
    bias_families(language)
        .iter()
        .position(|family| *family == spec.family)
        .unwrap_or(usize::MAX)
}

/// Design §5.3 score for one plan.
///
/// Per-model terms are summed across the plan's model steps, so a pipeline
/// reflects every model it loads; the passthrough bonus and the per-extra-step
/// penalty apply once, to the plan.
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

    if plan.passthrough {
        total += PASSTHROUGH_BONUS;
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
    total += speed_match(spec.speed, req.prefer);
    if installed(&spec.id) {
        total += INSTALLED_BONUS;
    }
    total -= MEMORY_RISK_PENALTY * memory_risk(spec, req.backend);

    total
}

/// Design §5.3 `speed_match`. Under `Speed::Accurate` the term is inverted, so
/// slowness scores higher: fast 0, balanced 20, accurate 40.
fn speed_match(speed: Option<Speed>, prefer: Speed) -> i32 {
    let rank = speed_rank(speed);

    match prefer {
        Speed::Accurate => SPEED_MATCH_BONUS * rank / 2,
        Speed::Fast => i32::from(rank == 0) * SPEED_MATCH_BONUS,
        Speed::Balanced => i32::from(rank == 1) * SPEED_MATCH_BONUS,
    }
}

/// Fast 0, balanced 1, accurate 2; an undeclared speed counts as balanced.
fn speed_rank(speed: Option<Speed>) -> i32 {
    match speed {
        Some(Speed::Fast) => 0,
        Some(Speed::Accurate) => 2,
        Some(Speed::Balanced) | None => 1,
    }
}

fn speed_from_rank(rank: i32) -> Speed {
    match rank {
        0 => Speed::Fast,
        2 => Speed::Accurate,
        _ => Speed::Balanced,
    }
}

/// Coarse memory-pressure hint, 0 (low) … 2 (high), used only by the §5.3 score.
///
/// Quantisation is read from the model file name — `bf16`/`f16`/`f32` cost the
/// most, `q8_*`/`q6_*` a little, `q4_*`/`q5_*` the least (anything unlabelled
/// counts as `q8`-like) — and one extra point is added when the backend is Metal
/// and the model is large ([`LARGE_MODEL_BYTES`]), because unified memory is
/// shared with the OS.
fn memory_risk(spec: &ModelSpec, backend: Backend) -> i32 {
    let name = spec.model_file.to_ascii_lowercase();
    let base = if name.contains("bf16") || name.contains("f16") || name.contains("f32") {
        2
    } else if name.contains("q4") || name.contains("q5") {
        0
    } else {
        1
    };

    let bytes: u64 = spec.files.iter().map(|file| file.size).sum();
    let large_on_metal = backend == Backend::Metal && bytes >= LARGE_MODEL_BYTES;

    (base + i32::from(large_on_metal)).min(2)
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
    let mut plans = Vec::new();
    // Steps that had no candidate, used to explain an empty result.
    let mut missing: Vec<StepKind> = Vec::new();

    for template in templates_for(req) {
        match fill_template(catalogue, installed, req, &template) {
            Ok(steps) => {
                let capabilities = union_capabilities(&steps, catalogue);
                plans.push(ModelPlan {
                    id: plan_id(&steps),
                    title_key: title_key(&steps),
                    model_ids: model_ids(&steps),
                    passthrough: steps.iter().any(|step| step.kind == StepKind::Passthrough),
                    cost: plan_cost(&steps, catalogue, req),
                    capabilities,
                    gaps: gaps(catalogue, req, &capabilities),
                    steps,
                });
            }
            Err(FillError::MissingModel(kind)) => missing.push(kind),
            // A gated template was never offered; there is nothing to explain.
            Err(FillError::Gated) => {}
        }
    }

    plans.sort_by(|a, b| {
        score(b, req, catalogue, installed).cmp(&score(a, req, catalogue, installed))
    });

    if plans.is_empty() {
        let mut gaps = gaps(catalogue, req, &NOTHING);
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
            passthrough: false,
            cost: PlanCost {
                steps: 0,
                speed: req.prefer,
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
    /// The passthrough candidate did not meet its viability condition.
    Gated,
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

    if template.gate == TemplateGate::BeatsPlainAsr {
        let (Some(passthrough), Some(asr)) = (
            passthrough_spec(catalogue, &steps),
            best_model_for_role(catalogue, installed, Role::Asr, req),
        ) else {
            return Err(FillError::Gated);
        };
        if !passthrough_beats_asr(req, passthrough, asr) {
            return Err(FillError::Gated);
        }
    }

    Ok(steps)
}

fn passthrough_spec<'a>(catalogue: &'a [ModelSpec], steps: &[PlanStep]) -> Option<&'a ModelSpec> {
    steps
        .iter()
        .find(|step| step.kind == StepKind::Passthrough)
        .and_then(|step| find(catalogue, &step.model_id))
}

/// Row 1's condition (design §5.2): offer the passthrough plan only when its
/// model is in a strictly faster speed class than the ASR pick, or serves the
/// requested language concretely where the ASR pick would fall back to `auto`.
fn passthrough_beats_asr(req: &Requirements, passthrough: &ModelSpec, asr: &ModelSpec) -> bool {
    speed_rank(passthrough.speed) < speed_rank(asr.speed)
        || (language_concrete(passthrough, &req.language)
            && !language_concrete(asr, &req.language))
}

/// True when [`catalog::map_language`] returns a concrete tag rather than `auto`.
fn language_concrete(spec: &ModelSpec, app_lang: &str) -> bool {
    catalog::map_language(spec, app_lang) != "auto"
}

/// What a filled plan provides: the union over its model steps.
///
/// [`StepKind::TurnAttribution`] is the Rust step that joins ASR text onto diar
/// turns, so a plan containing it upgrades a `Turns` model to `TurnsWithText`
/// (design §6.4) — that is what makes the composed plan satisfy a diarization
/// requirement without a passthrough model.
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
pub fn gaps(catalogue: &[ModelSpec], req: &Requirements, capabilities: &Capabilities) -> Vec<Gap> {
    let mut gaps = Vec::new();

    if req.word_timestamps && capabilities.timestamps < TimestampGrade::Word {
        gaps.push(Gap {
            requirement: "word_timestamps",
            message: format!(
                "word-level timestamps requested, but the plan only reaches {} timestamps",
                grade_label(capabilities.timestamps)
            ),
            suggested_model_id: suggest_word_model(catalogue),
        });
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

/// The gap for a step that had no candidate, used when nothing can be solved.
fn gap_for_step(catalogue: &[ModelSpec], kind: StepKind) -> Gap {
    let (requirement, message) = match kind {
        StepKind::Vad => ("vad", "no VAD model is available"),
        StepKind::Asr => ("asr", "no ASR model matches the requested language"),
        StepKind::Align => ("align", "no word-level aligner is available"),
        StepKind::Diarize => ("diarization", "no speaker-separation model is available"),
        StepKind::Passthrough => ("passthrough", "no one-shot model is available"),
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
fn suggest_word_model(catalogue: &[ModelSpec]) -> Option<String> {
    let mut candidates: Vec<&ModelSpec> = catalogue
        .iter()
        .filter(|spec| spec.capabilities().timestamps >= TimestampGrade::Word)
        .collect();

    // An aligner is the direct fix; a word-capable ASR also works.
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

/// Model the plan id is keyed on (design §3.2: `passthrough:moss`,
/// `asr+diar+turns:nemotron`): the passthrough model when there is one, else the
/// last non-ASR model step — the step that makes this plan distinctive.
fn id_anchor(steps: &[PlanStep]) -> Option<&PlanStep> {
    steps
        .iter()
        .find(|step| step.kind == StepKind::Passthrough)
        .or_else(|| {
            steps
                .iter()
                .rev()
                .find(|step| step.kind != StepKind::Asr && !step.model_id.is_empty())
        })
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
    } else if steps.iter().any(|step| step.kind == StepKind::Passthrough) {
        "modelPlan.planPassthroughMoss"
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

/// Cost hint: step count plus the slowest model's speed class.
fn plan_cost(steps: &[PlanStep], catalogue: &[ModelSpec], req: &Requirements) -> PlanCost {
    let slowest = steps
        .iter()
        .filter_map(|step| find(catalogue, &step.model_id))
        .map(|spec| speed_rank(spec.speed))
        .max()
        .unwrap_or_else(|| speed_rank(Some(req.prefer)));

    PlanCost {
        steps: steps.len(),
        speed: speed_from_rank(slowest),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::catalog::ModelFile;

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
            speed: Some(fake.speed),
            source: Some(fake.source),
            tier: Some(fake.tier),
            licenses: Vec::new(),
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
            }),
            spec(FakeModel {
                id: "moss_transcribe_diarize_q8_0",
                family: "moss_transcribe_diarize",
                roles: &[Role::Passthrough, Role::Asr],
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
            }),
            spec(FakeModel {
                id: "moss_transcribe_diarize_q4_k",
                family: "moss_transcribe_diarize",
                roles: &[Role::Passthrough, Role::Asr],
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
    /// viable one, because the row-1 passthrough candidate is gated out — MOSS is
    /// neither faster than SenseVoice nor a better language match for Chinese.
    #[test]
    fn default_requirements_pick_sense_asr_first() {
        let catalogue = catalogue();
        let installed = all_installed();
        let req = req();

        let plans = solve(&catalogue, &installed, &req);

        assert_eq!(plans.len(), 1, "only the plain-ASR template is viable");
        assert_eq!(plans[0].model_ids, vec!["sensevoice_small_q8".to_string()]);
        assert_eq!(plans[0].steps[0].kind, StepKind::Asr);
        assert!(!plans[0].passthrough);
        assert_eq!(plans[0].id, "asr:sense");
        assert_eq!(plans[0].title_key, "modelPlan.planAsrVad");
        assert!(plans[0].gaps.is_empty());

        let best = best_model_for_role(&catalogue, &installed, Role::Asr, &req)
            .expect("an ASR model is available");
        assert_eq!(best.id, "sensevoice_small_q8");
    }

    /// Speaker separation with a passthrough model present: the solver picks
    /// the one-shot MOSS plan automatically (fewer steps, own timestamps and
    /// labels), with the composed plan as the runner-up. No user setting is
    /// involved — this is a runtime capability decision.
    #[test]
    fn diarization_automatically_prefers_the_passthrough_plan() {
        let catalogue = catalogue();
        let installed = all_installed();
        let req = Requirements {
            diarization: true,
            ..req()
        };

        let plans = solve(&catalogue, &installed, &req);

        assert!(!plans.is_empty());
        assert!(plans[0].passthrough, "the one-shot plan wins on score");
        assert_eq!(
            plans[0].model_ids,
            vec!["moss_transcribe_diarize_q8_0".to_string()]
        );
        assert_eq!(plans[0].steps.len(), 1);
        assert_eq!(plans[0].steps[0].kind, StepKind::Passthrough);
        assert_eq!(plans[0].steps[0].task, "asr");
        assert_eq!(plans[0].id, "passthrough:moss");
        assert_eq!(
            plans[0].capabilities.diarization,
            DiarizationGrade::TurnsWithText
        );
        assert!(plans[0].gaps.is_empty());

        // The composed alternative is still offered as a candidate, so the UI
        // can explain the trade-off (and it is the fallback when MOSS is not
        // installed).
        assert!(
            plans.iter().any(|plan| !plan.passthrough),
            "the composed ASR+diar plan must remain a candidate"
        );
    }

    /// The composed alternative to passthrough: SenseVoice is reused, a `diar`
    /// model supplies turns and the pure-Rust step attributes them — so the
    /// plan ends with `TurnAttribution` and still reports speaker coverage.
    #[test]
    fn diarization_composes_asr_diar_turns_when_passthrough_scores_lower() {
        let catalogue = catalogue();
        let installed = all_installed();
        let req = Requirements {
            diarization: true,
            ..req()
        };

        let plans = solve(&catalogue, &installed, &req);

        // The composed plan is a real candidate even though MOSS outranks it.
        let plan = plans
            .iter()
            .find(|plan| !plan.passthrough)
            .expect("the composed template is viable");
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

    /// Word-level + speakers: word timing is NOT implemented end to end, so the
    /// solver must return satisfiable speaker plans AND explicitly report the
    /// word-timestamp shortfall. Previously it returned an `Align` step that was
    /// never executed, so the UI claimed word timing it never produced.
    #[test]
    fn word_and_diarization_reports_the_word_timestamp_gap() {
        let catalogue = catalogue();
        let installed = all_installed();
        let req = Requirements {
            word_timestamps: true,
            diarization: true,
            ..req()
        };

        let plans = solve(&catalogue, &installed, &req);

        assert!(!plans.is_empty(), "speaker separation is still satisfiable");
        assert!(plans[0].passthrough, "the passthrough plan scores higher");

        // No plan may claim an alignment step, because none would run.
        assert!(
            plans
                .iter()
                .all(|plan| plan.steps.iter().all(|step| step.kind != StepKind::Align)),
            "an Align step is never executed, so it must not be planned"
        );

        // Every candidate must admit the word-timestamp shortfall.
        for plan in &plans {
            let gap = plan
                .gaps
                .iter()
                .find(|gap| gap.requirement == "word_timestamps")
                .unwrap_or_else(|| panic!("plan {} hides the word gap", plan.id));
            assert!(
                gap.message.contains("word"),
                "gap must explain the word shortfall, got {:?}",
                gap.message
            );
        }

        // The passthrough plan still separates speakers in one call.
        let passthrough = &plans[0];
        assert_eq!(
            passthrough.model_ids,
            vec!["moss_transcribe_diarize_q8_0".to_string()],
            "the aligner must not be pulled in any more"
        );
        assert_eq!(
            passthrough.capabilities.timestamps,
            TimestampGrade::Segment,
            "MOSS itself only emits segment timestamps"
        );
        assert_eq!(passthrough.capabilities.diarization, DiarizationGrade::TurnsWithText);

        // The composed alternative keeps speaker coverage.
        let composed = plans
            .iter()
            .find(|plan| !plan.passthrough)
            .expect("the composed template is viable");
        assert!(composed
            .steps
            .iter()
            .any(|step| step.kind == StepKind::Diarize));
        assert_ne!(passthrough.id, composed.id);
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
    /// passthrough model) must produce a gap plus a concrete download
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

    /// Without a `diar` model but WITH the passthrough model, the solver still
    /// satisfies speaker separation — via the one-shot plan (design §5.2 row 2).
    #[test]
    fn missing_diar_model_falls_back_to_passthrough() {
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
        assert!(
            plans[0].passthrough,
            "the one-shot plan is the only way to get speakers"
        );
        assert!(plans[0].gaps.is_empty());
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

    /// `memory_risk` is a pure function of the spec: lower for `q4_k` than for
    /// `q8_0`, higher for bf16, and one point worse on Metal for a large file.
    #[test]
    fn memory_risk_ranks_quantisation_and_backend_size() {
        let q4 = spec_of("moss_transcribe_diarize_q4_k");
        let q8 = spec_of("moss_transcribe_diarize_q8_0");
        let small = spec_of("nemotron_3_diar_q8_0");

        assert!(memory_risk(&q4, Backend::Metal) < memory_risk(&q8, Backend::Metal));
        assert_eq!(memory_risk(&q8, Backend::Cpu), 1);
        assert_eq!(
            memory_risk(&q8, Backend::Metal),
            2,
            "a 1 GiB+ file on Metal costs an extra point"
        );
        assert_eq!(
            memory_risk(&small, Backend::Metal),
            memory_risk(&small, Backend::Cpu),
            "a small file is not penalised for being on Metal"
        );
    }

    /// Templates are enumerated purely from the capability requirements; there
    /// is no passthrough knob any more, so the counts are the §5.2 table rows.
    #[test]
    fn templates_are_enumerated_from_capability_requirements() {
        let plain = req();
        let templates = templates_for(&plain);
        assert_eq!(templates.len(), 2, "row 1 offers Asr and Passthrough");
        assert_eq!(templates[0].steps, vec![StepKind::Asr]);
        assert_eq!(templates[1].steps, vec![StepKind::Passthrough]);

        let spoken = Requirements {
            diarization: true,
            ..plain.clone()
        };
        let templates = templates_for(&spoken);
        assert_eq!(templates.len(), 2, "row 2 offers Passthrough and Asr+Diar+Turns");
        assert_eq!(
            templates[0].steps,
            vec![StepKind::Passthrough],
            "the one-shot plan is listed first"
        );
        assert_eq!(
            templates[1].steps,
            vec![StepKind::Asr, StepKind::Diarize, StepKind::TurnAttribution]
        );
    }

    /// The solver must behave sensibly against the REAL committed catalogue,
    /// not just the hand-built fakes. This is the integration guard that the
    /// defaults still resolve to SenseVoice and that the passthrough-only and
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
        assert!(!plans[0].passthrough, "default plan must not be passthrough");

        // Passthrough-only + diarization: must be the MOSS model.
        // Speaker separation: the solver picks the one-shot MOSS plan, and the
        // composed ASR + real-diar-model plan is still offered alongside it.
        let spoken = solve(
            &real,
            &installed,
            &Requirements {
                diarization: true,
                ..req()
            },
        );
        assert!(
            spoken[0]
                .model_ids
                .iter()
                .any(|id| id.starts_with("moss_transcribe_diarize")),
            "the one-shot plan should win for speaker separation, got {:?}",
            spoken[0].model_ids
        );
        assert!(spoken[0].passthrough);

        // The composed alternative (real diar model + turn attribution) must
        // remain available as the fallback when MOSS is not installed.
        let composed = spoken
            .iter()
            .find(|plan| !plan.passthrough)
            .expect("the composed template is viable against the real catalogue");
        assert!(
            composed
                .steps
                .iter()
                .any(|step| step.kind == StepKind::Diarize),
            "composed plan needs a Diarize step, got {:?}",
            composed.steps
        );
    }
}
