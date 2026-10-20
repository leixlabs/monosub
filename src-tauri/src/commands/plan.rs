//! `resolve_model_plan` — expose the requirement → ModelPlan solver to the UI.
//!
//! Task: M3. See docs/tech-spec/model-selection-design.md §5 / §7.5 and
//! docs/tech-spec/model-selection-tasks.md M3.
//!
//! The solver itself ([`crate::engine::plan::solve`]) is pure and unit-tested
//! without a Tauri handle. This module is the thin command shell around it: it
//! reads the catalogue plus install state from the app, maps the wire request
//! (all fields optional, design §9.1) onto [`Requirements`], and returns a
//! camelCase DTO the frontend consumes directly.
//!
//! The mapping helpers ([`passthrough_from_wire`], [`speed_from_wire`],
//! [`backend_from_device`]) are deliberately pure so they can be tested without
//! constructing an `AppHandle`.

use serde::{Deserialize, Serialize};
use tauri::AppHandle;

use crate::engine::catalog::{self, Capabilities, DiarizationGrade, Speed, TimestampGrade};
use crate::engine::plan::{
    self, Gap, LanguageProfile, MemoryBudget, ModelPlan, PlanCost, PlanStep, Requirements,
    StepKind,
};
use crate::engine::Backend;

/// Wire request for `resolve_model_plan`. Every field is optional so an older
/// frontend that sends nothing still gets the documented defaults.
///
/// There is deliberately no passthrough field: whether a single model can emit
/// text + timestamps + speakers is decided by the solver at runtime.
///
/// The accepted spelling is **camelCase**, matching the camelCase response DTOs
/// and `src/lib/modelPlan.ts`. The snake_case spellings from design §9.1 are
/// kept as aliases, because they were the documented wire format before this
/// command's request was aligned with its own response.
///
/// Regression: this struct previously declared `rename_all = "snake_case"`
/// while the frontend sent camelCase. Serde silently ignores unknown fields, so
/// every requirement arrived as `None` and the Settings preview rendered the
/// same default plan no matter which toggle the user flipped.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanRequirementsRequest {
    #[serde(alias = "require_word_timestamps")]
    pub require_word_timestamps: Option<bool>,
    #[serde(alias = "enable_diarization")]
    pub enable_diarization: Option<bool>,
    pub language: Option<String>,
    /// 语种场景: `mandarin` / `mixed` / `multilingual`.
    #[serde(alias = "language_profile")]
    pub language_profile: Option<String>,
    /// 显存占用: `small` / `medium` / `large`.
    #[serde(alias = "memory_budget")]
    pub memory_budget: Option<String>,
    pub device: Option<String>,
    /// Manual `role -> model_id` pins (design §8.2.3), same spelling as
    /// `TranscribeRequest.model_overrides` so one store slice drives both.
    #[serde(alias = "model_overrides")]
    pub model_overrides: Option<std::collections::HashMap<String, String>>,
}

/// One executable plan, flattened for the frontend.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanStepDto {
    pub kind: &'static str,
    pub model_id: String,
    pub task: String,
    pub family: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanCostDto {
    pub steps: usize,
    pub speed: &'static str,
    /// 显存 class the plan's own weight files imply (`small`/`medium`/`large`).
    pub memory: &'static str,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CapabilitiesDto {
    pub timestamps: &'static str,
    pub diarization: &'static str,
    pub produces_text: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GapDto {
    pub requirement: &'static str,
    pub message: String,
    pub suggested_model_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelPlanDto {
    pub id: String,
    pub title_key: &'static str,
    pub model_ids: Vec<String>,
    pub steps: Vec<PlanStepDto>,
    pub passthrough: bool,
    pub cost: PlanCostDto,
    pub capabilities: CapabilitiesDto,
    pub gaps: Vec<GapDto>,
}

/// `resolve_model_plan` payload: the chosen plan, every candidate, and the gaps.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanResponse {
    pub plan: ModelPlanDto,
    pub candidates: Vec<ModelPlanDto>,
    pub gaps: Vec<GapDto>,
}

/// Map `small` / `medium` / `large`; anything else is `Medium`.
///
/// Kept as a thin wrapper over [`MemoryBudget::from_wire`] so the three
/// spellings live in exactly one place — the solver module, which the Settings
/// preview and the transcription run both call.
pub fn memory_from_wire(value: Option<&str>) -> MemoryBudget {
    MemoryBudget::from_wire(value)
}

/// Map `mandarin` / `mixed` / `multilingual`; anything else is `Mandarin`.
pub fn language_profile_from_wire(value: Option<&str>) -> LanguageProfile {
    LanguageProfile::from_wire(value)
}

/// Same rule as `transcribe::resolve_backend`: only an explicit `cpu` forces
/// CPU, everything else is Metal on macOS and CPU elsewhere.
pub fn backend_from_device(device: Option<&str>) -> Backend {
    match device.map(|value| value.trim().to_lowercase()) {
        Some(value) if value == "cpu" => Backend::Cpu,
        _ if cfg!(target_os = "macos") => Backend::Metal,
        _ => Backend::Cpu,
    }
}

impl PlanRequirementsRequest {
    /// Pure conversion into the solver's [`Requirements`].
    pub fn to_requirements(&self) -> Requirements {
        Requirements {
            word_timestamps: self.require_word_timestamps.unwrap_or(false),
            diarization: self.enable_diarization.unwrap_or(false),
            language: self.language.clone().unwrap_or_else(|| "zh".to_string()),
            language_profile: language_profile_from_wire(self.language_profile.as_deref()),
            memory: memory_from_wire(self.memory_budget.as_deref()),
            backend: backend_from_device(self.device.as_deref()),
        }
    }
}

fn step_kind_name(kind: StepKind) -> &'static str {
    match kind {
        StepKind::Vad => "vad",
        StepKind::Asr => "asr",
        StepKind::Align => "align",
        StepKind::Diarize => "diarize",
        StepKind::Passthrough => "passthrough",
        StepKind::TurnAttribution => "turn_attribution",
    }
}

fn speed_name(speed: Speed) -> &'static str {
    speed.as_str()
}

fn timestamp_name(grade: TimestampGrade) -> &'static str {
    match grade {
        TimestampGrade::None => "none",
        TimestampGrade::Turns => "turns",
        TimestampGrade::Segment => "segment",
        TimestampGrade::Word => "word",
    }
}

fn diarization_name(grade: DiarizationGrade) -> &'static str {
    match grade {
        DiarizationGrade::None => "none",
        DiarizationGrade::Turns => "turns",
        DiarizationGrade::TurnsWithText => "turns_with_text",
    }
}

impl From<Capabilities> for CapabilitiesDto {
    fn from(caps: Capabilities) -> Self {
        Self {
            timestamps: timestamp_name(caps.timestamps),
            diarization: diarization_name(caps.diarization),
            produces_text: caps.produces_text,
        }
    }
}

impl From<PlanStep> for PlanStepDto {
    fn from(step: PlanStep) -> Self {
        Self {
            kind: step_kind_name(step.kind),
            model_id: step.model_id,
            task: step.task,
            family: step.family,
        }
    }
}

impl From<PlanCost> for PlanCostDto {
    fn from(cost: PlanCost) -> Self {
        Self {
            steps: cost.steps,
            speed: speed_name(cost.speed),
            memory: cost.memory.label(),
        }
    }
}

impl From<Gap> for GapDto {
    fn from(gap: Gap) -> Self {
        Self {
            requirement: gap.requirement,
            message: gap.message,
            suggested_model_id: gap.suggested_model_id,
        }
    }
}

impl From<ModelPlan> for ModelPlanDto {
    fn from(plan: ModelPlan) -> Self {
        Self {
            id: plan.id,
            title_key: plan.title_key,
            model_ids: plan.model_ids,
            steps: plan.steps.into_iter().map(PlanStepDto::from).collect(),
            passthrough: plan.passthrough,
            cost: plan.cost.into(),
            capabilities: plan.capabilities.into(),
            gaps: plan.gaps.into_iter().map(GapDto::from).collect(),
        }
    }
}

/// Solve the requirement → plan problem for the current catalogue and install
/// state (design §5.1 / §7.5).
///
/// The frontend calls this whenever a requirement toggle changes, so the
/// Settings preview is always live. Resolution never fails for a missing model:
/// the response carries a gap-bearing plan instead (design §5.6).
#[tauri::command]
pub async fn resolve_model_plan(
    app: AppHandle,
    requirements: Option<PlanRequirementsRequest>,
) -> Result<PlanResponse, String> {
    let request = requirements.unwrap_or_default();
    let req = request.to_requirements();

    let specs = catalog::catalog(Some(&app)).map_err(|e| e.to_string())?;
    let installed = |id: &str| catalog::is_installed(&app, id);

    let mut candidates = plan::solve(&specs, &installed, &req);

    // Manual pins are applied to the winning plan only, exactly as
    // `commands::transcribe` does at run time, so the preview shows the plan
    // the run will actually use. The remaining candidates stay the solver's
    // ranking, which is what the "why this one?" list explains.
    if let Some(overrides) = request.model_overrides.as_ref() {
        plan::apply_model_overrides(&mut candidates, overrides, &specs);
    }

    tracing::debug!(
        word_timestamps = req.word_timestamps,
        diarization = req.diarization,
        language = %req.language,
        language_profile = ?req.language_profile,
        memory = req.memory.label(),
        backend = ?req.backend,
        candidates = candidates.len(),
        chosen = candidates.first().map(|plan| plan.id.as_str()).unwrap_or("<none>"),
        "resolved model plan"
    );

    // `solve` returns a gap-bearing empty plan when nothing fits, so the first
    // entry is always a valid payload; an empty list is still handled.
    let plan_dto = candidates
        .first()
        .cloned()
        .map(ModelPlanDto::from)
        .unwrap_or_else(|| ModelPlanDto {
            id: "unsolved".to_string(),
            title_key: "modelPlan.gapTitle",
            model_ids: Vec::new(),
            steps: Vec::new(),
            passthrough: false,
            cost: PlanCostDto {
                steps: 0,
                speed: speed_name(Speed::Balanced),
                memory: req.memory.label(),
            },
            capabilities: Capabilities {
                timestamps: TimestampGrade::None,
                diarization: DiarizationGrade::None,
                produces_text: false,
            }
            .into(),
            gaps: Vec::new(),
        });

    let gaps = plan_dto.gaps.clone();

    Ok(PlanResponse {
        plan: plan_dto,
        candidates: candidates.into_iter().map(ModelPlanDto::from).collect(),
        gaps,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_and_profile_and_backend_mapping() {
        assert_eq!(memory_from_wire(Some("small")), MemoryBudget::Small);
        assert_eq!(memory_from_wire(Some("LARGE")), MemoryBudget::Large);
        assert_eq!(memory_from_wire(Some("nonsense")), MemoryBudget::Medium);
        assert_eq!(memory_from_wire(None), MemoryBudget::Medium);

        assert_eq!(
            language_profile_from_wire(Some("mixed")),
            LanguageProfile::Mixed
        );
        assert_eq!(
            language_profile_from_wire(Some("Multilingual")),
            LanguageProfile::Multilingual
        );
        assert_eq!(
            language_profile_from_wire(Some("nonsense")),
            LanguageProfile::Mandarin
        );
        assert_eq!(language_profile_from_wire(None), LanguageProfile::Mandarin);

        assert_eq!(backend_from_device(Some("cpu")), Backend::Cpu);
        assert_eq!(
            backend_from_device(Some("metal")),
            if cfg!(target_os = "macos") {
                Backend::Metal
            } else {
                Backend::Cpu
            }
        );
    }

    #[test]
    fn empty_request_uses_documented_defaults() {
        let req = PlanRequirementsRequest::default().to_requirements();
        assert!(!req.word_timestamps);
        assert!(!req.diarization);
        assert_eq!(req.language, "zh");
        assert_eq!(req.memory, MemoryBudget::Medium);
        assert_eq!(req.language_profile, LanguageProfile::Mandarin);
    }

    #[test]
    fn request_fields_are_honoured() {
        let req = PlanRequirementsRequest {
            require_word_timestamps: Some(true),
            enable_diarization: Some(true),
            language: Some("ko".to_string()),
            language_profile: Some("mixed".to_string()),
            memory_budget: Some("small".to_string()),
            device: Some("cpu".to_string()),
            model_overrides: None,
        }
        .to_requirements();

        assert!(req.word_timestamps);
        assert!(req.diarization);
        assert_eq!(req.language, "ko");
        assert_eq!(req.language_profile, LanguageProfile::Mixed);
        assert_eq!(req.memory, MemoryBudget::Small);
        assert_eq!(req.backend, Backend::Cpu);
    }

    /// Regression: the frontend (`src/lib/modelPlan.ts`) sends **camelCase**,
    /// matching the camelCase DTOs this command returns. The struct once
    /// declared `rename_all = "snake_case"`, and serde silently drops unknown
    /// fields — so every toggle the user flipped arrived as `None` and the
    /// Settings preview showed the same default plan forever.
    #[test]
    fn wire_request_accepts_the_camel_case_the_frontend_sends() {
        let request: PlanRequirementsRequest = serde_json::from_value(serde_json::json!({
            "requireWordTimestamps": true,
            "enableDiarization": true,
            "language": "zh",
            "languageProfile": "mixed",
            "memoryBudget": "small",
            "device": "cpu",
            "modelOverrides": { "asr": "qwen3_asr_0_6b_q8_0" },
        }))
        .expect("camelCase request deserializes");

        assert_eq!(request.require_word_timestamps, Some(true));
        assert_eq!(request.enable_diarization, Some(true));
        assert_eq!(request.language_profile.as_deref(), Some("mixed"));
        assert_eq!(request.memory_budget.as_deref(), Some("small"));
        assert_eq!(
            request
                .model_overrides
                .as_ref()
                .and_then(|overrides| overrides.get("asr"))
                .map(String::as_str),
            Some("qwen3_asr_0_6b_q8_0")
        );

        let req = request.to_requirements();
        assert!(req.word_timestamps, "the toggle must reach the solver");
        assert!(req.diarization, "the toggle must reach the solver");
        assert_eq!(req.language_profile, LanguageProfile::Mixed);
        assert_eq!(req.memory, MemoryBudget::Small);
    }

    /// The snake_case spellings from design §9.1 keep working as aliases, so a
    /// stored blob or an older caller does not silently lose its requirements.
    #[test]
    fn wire_request_still_accepts_the_documented_snake_case_aliases() {
        let request: PlanRequirementsRequest = serde_json::from_value(serde_json::json!({
            "require_word_timestamps": true,
            "enable_diarization": true,
            "language_profile": "multilingual",
            "memory_budget": "large",
            "model_overrides": { "diar": "sortformer_diar_4spk_q8_0" },
        }))
        .expect("snake_case aliases deserialize");

        assert_eq!(request.require_word_timestamps, Some(true));
        assert_eq!(request.enable_diarization, Some(true));
        assert_eq!(request.language_profile.as_deref(), Some("multilingual"));
        assert_eq!(request.memory_budget.as_deref(), Some("large"));
        assert!(request.model_overrides.is_some());
    }

    /// The preview and the run must apply pinning identically, so the command
    /// shares `plan::apply_model_overrides` rather than keeping its own copy.
    #[test]
    fn overrides_pin_the_previewed_plan() {
        let catalogue = vec![crate::engine::catalog::ModelSpec {
            id: "sortformer_diar_4spk_q8_0".to_string(),
            family: "sortformer_diar".to_string(),
            display_name: "Sortformer".to_string(),
            target_directory: "d".to_string(),
            model_file: "m.gguf".to_string(),
            files: Vec::new(),
            languages: vec!["auto".to_string()],
            license: "CC-BY-NC-4.0".to_string(),
            attribution: None,
            capabilities_raw: None,
            roles: Vec::new(),
            engine: crate::engine::catalog::Engine::default(),
            prompt_template: None,
            speed: None,
            source: None,
            tier: None,
            licenses: Vec::new(),
            requires: Vec::new(),
            language_tokens: Vec::new(),
        }];

        let mut plans = plan::solve(&catalogue, &|_: &str| true, &Requirements::default());
        assert!(!plans[0].steps.is_empty(), "there is a plan to pin onto");

        let mut overrides = std::collections::HashMap::new();
        overrides.insert("diar".to_string(), "sortformer_diar_4spk_q8_0".to_string());
        plan::apply_model_overrides(&mut plans, &overrides, &catalogue);

        assert!(
            plans[0].steps.iter().any(|step| step.kind == StepKind::Diarize
                && step.model_id == "sortformer_diar_4spk_q8_0"),
            "the pin must show up in the previewed plan: {:?}",
            plans[0].steps
        );
        assert!(plans[0].model_ids.contains(&"sortformer_diar_4spk_q8_0".to_string()));
    }

    #[test]
    fn dto_mapping_preserves_step_and_gap_fields() {
        let plan = ModelPlan {
            id: "passthrough:moss".to_string(),
            title_key: "modelPlan.planPassthroughMoss",
            model_ids: vec!["moss_transcribe_diarize_q8_0".to_string()],
            steps: vec![PlanStep {
                kind: StepKind::Passthrough,
                model_id: "moss_transcribe_diarize_q8_0".to_string(),
                task: "asr".to_string(),
                family: "moss_transcribe_diarize".to_string(),
            }],
            passthrough: true,
            cost: PlanCost {
                steps: 1,
                speed: Speed::Balanced,
                memory: MemoryBudget::Medium,
            },
            capabilities: Capabilities {
                timestamps: TimestampGrade::Segment,
                diarization: DiarizationGrade::TurnsWithText,
                produces_text: true,
            },
            gaps: vec![Gap {
                requirement: "word_timestamps",
                message: "needs word-level timing".to_string(),
                suggested_model_id: None,
            }],
        };

        let dto = ModelPlanDto::from(plan);
        assert_eq!(dto.id, "passthrough:moss");
        assert_eq!(dto.steps[0].kind, "passthrough");
        assert_eq!(dto.steps[0].task, "asr");
        assert_eq!(dto.cost.speed, "balanced");
        assert_eq!(dto.cost.memory, "medium");
        assert_eq!(dto.capabilities.diarization, "turns_with_text");
        assert_eq!(dto.gaps[0].requirement, "word_timestamps");
        assert!(dto.passthrough);
    }
}
