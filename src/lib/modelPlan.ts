import { invoke } from '@tauri-apps/api/core';

/**
 * Frontend wrapper over the requirement → ModelPlan solver command
 * (`resolve_model_plan`; docs/tech-spec/model-selection-design.md §5 / §7.5).
 *
 * The Rust side returns camelCase DTOs, so the types below mirror them directly.
 * Every request field is optional; the Rust defaults are word_timestamps=false,
 * diarization=false, language=zh, speed=balanced.
 */

/**
 * Whether the chosen plan uses a single call that emits text + timestamps +
 * speakers. Reported by the solver; there is no user setting for it.
 */
export type PlanPassthrough = boolean;

export type SpeedPreference = 'fast' | 'balanced' | 'accurate';

export type PlanStepKind =
  | 'vad'
  | 'asr'
  | 'diarize'
  | 'passthrough'
  | 'turn_attribution';

export type TimestampGradeName = 'none' | 'turns' | 'segment' | 'word';
export type DiarizationGradeName = 'none' | 'turns' | 'turns_with_text';

export interface PlanRequirements {
  requireWordTimestamps?: boolean;
  enableDiarization?: boolean;
  language?: string;
  speedPreference?: SpeedPreference;
  device?: string;
}

export interface PlanStep {
  kind: PlanStepKind;
  modelId: string;
  task: string;
  family: string;
}

export interface PlanCost {
  steps: number;
  speed: SpeedPreference;
}

export interface PlanCapabilities {
  timestamps: TimestampGradeName;
  diarization: DiarizationGradeName;
  producesText: boolean;
}

export interface PlanGap {
  requirement: string;
  message: string;
  suggestedModelId: string | null;
}

export interface ModelPlan {
  id: string;
  titleKey: string;
  modelIds: string[];
  steps: PlanStep[];
  passthrough: boolean;
  cost: PlanCost;
  capabilities: PlanCapabilities;
  gaps: PlanGap[];
}

export interface PlanResponse {
  /** The chosen plan (candidates[0], or a gap-bearing empty plan). */
  plan: ModelPlan;
  /** Every candidate, best first, for the "why this one?" explanation. */
  candidates: ModelPlan[];
  /** Convenience copy of `plan.gaps`. */
  gaps: PlanGap[];
}

/**
 * Solve the requirement → plan problem against the installed catalogue.
 *
 * Never rejects for a missing model: the response carries a gap-bearing plan
 * so the UI can offer a download instead (design §5.6).
 */
export const resolveModelPlan = (
  requirements: PlanRequirements = {},
): Promise<PlanResponse> =>
  invoke<PlanResponse>('resolve_model_plan', { requirements });

/** True when the plan needs at least one model the user has not installed. */
export const planHasGaps = (plan: ModelPlan): boolean => plan.gaps.length > 0;

/** Model that would close the first gap, when the catalogue has one. */
export const firstGapSuggestion = (plan: ModelPlan): string | null =>
  plan.gaps.find((gap) => gap.suggestedModelId)?.suggestedModelId ?? null;
