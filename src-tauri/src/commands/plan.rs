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
    self, Gap, ModelPlan, PlanCost, PlanStep, Requirements, StepKind,
};
use crate::engine::Backend;

/// Wire request for `resolve_model_plan`. Every field is optional so an older
/// frontend that sends nothing still gets the documented defaults.
///
/// There is deliberately no passthrough field: whether a single model can emit
/// text + timestamps + speakers is decided by the solver at runtime.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct PlanRequirementsRequest {
    pub require_word_timestamps: Option<bool>,
    pub enable_diarization: Option<bool>,
    pub language: Option<String>,
    pub speed_preference: Option<String>,
    pub device: Option<String>,
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

/// Map `fast` / `balanced` / `accurate`; anything else is `Balanced`.
pub fn speed_from_wire(value: Option<&str>) -> Speed {
    match value.map(|value| value.trim().to_lowercase()).as_deref() {
        Some("fast") => Speed::Fast,
        Some("accurate") => Speed::Accurate,
        _ => Speed::Balanced,
    }
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
            prefer: speed_from_wire(self.speed_preference.as_deref()),
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
    match speed {
        Speed::Fast => "fast",
        Speed::Balanced => "balanced",
        Speed::Accurate => "accurate",
    }
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

    let candidates = plan::solve(&specs, &installed, &req);

    tracing::debug!(
        word_timestamps = req.word_timestamps,
        diarization = req.diarization,
        language = %req.language,
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
    fn speed_and_backend_mapping() {
        assert_eq!(speed_from_wire(Some("fast")), Speed::Fast);
        assert_eq!(speed_from_wire(Some("ACCURATE")), Speed::Accurate);
        assert_eq!(speed_from_wire(Some("nonsense")), Speed::Balanced);
        assert_eq!(speed_from_wire(None), Speed::Balanced);

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
        assert_eq!(req.prefer, Speed::Balanced);
    }

    #[test]
    fn request_fields_are_honoured() {
        let req = PlanRequirementsRequest {
            require_word_timestamps: Some(true),
            enable_diarization: Some(true),
            language: Some("ko".to_string()),
            speed_preference: Some("fast".to_string()),
            device: Some("cpu".to_string()),
        }
        .to_requirements();

        assert!(req.word_timestamps);
        assert!(req.diarization);
        assert_eq!(req.language, "ko");
        assert_eq!(req.prefer, Speed::Fast);
        assert_eq!(req.backend, Backend::Cpu);
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
        assert_eq!(dto.capabilities.diarization, "turns_with_text");
        assert_eq!(dto.gaps[0].requirement, "word_timestamps");
        assert!(dto.passthrough);
    }
}
