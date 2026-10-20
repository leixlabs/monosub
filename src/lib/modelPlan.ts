import { invoke } from '@tauri-apps/api/core';

/**
 * Frontend wrapper over the requirement → ModelPlan solver command
 * (`resolve_model_plan`; docs/tech-spec/model-selection-design.md §5 / §7.5).
 *
 * The Rust side returns camelCase DTOs, so the types below mirror them directly.
 * Every request field is optional; the Rust defaults are word_timestamps=false,
 * diarization=false, language=zh, language_profile=mandarin, memory=medium.
 *
 * There is no `speedPreference`: `memoryBudget` answers speed as well, because in
 * this catalogue the weight-file size decides both footprint and throughput.
 */

/**
 * Whether the chosen plan uses a single call that emits text + timestamps +
 * speakers. Reported by the solver; there is no user setting for it.
 */
export type PlanPassthrough = boolean;

/**
 * 语种场景 — mirrors `engine::plan::LanguageProfile`.
 *
 * `mandarin` is the default and keeps the per-language family table; `mixed` and
 * `multilingual` re-rank it toward the models that transcribe language switches.
 */
export type LanguageProfile = 'mandarin' | 'mixed' | 'multilingual';

/**
 * 显存占用 — mirrors `engine::plan::MemoryBudget`.
 *
 * Answers the speed question too: within this catalogue the weight-file size
 * decides both footprint and throughput, so one question replaces the old
 * `fast`/`balanced`/`accurate` preference.
 */
export type MemoryBudget = 'small' | 'medium' | 'large';

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
  /** 语种场景. Defaults to `mandarin` server-side. */
  languageProfile?: LanguageProfile;
  /** 显存占用. Defaults to `medium` server-side. */
  memoryBudget?: MemoryBudget;
  device?: string;
  /**
   * Manual `role -> model_id` pins (design §8.2.3).
   *
   * Sent so the preview shows the plan the run will actually execute: the
   * command applies these to the winning candidate exactly as
   * `commands::transcribe` does. An empty string means "no pin".
   */
  modelOverrides?: Partial<Record<'asr' | 'vad' | 'align' | 'diar', string>>;
}

export interface PlanStep {
  kind: PlanStepKind;
  modelId: string;
  task: string;
  family: string;
}

export interface PlanCost {
  steps: number;
  /**
   * The slowest model's speed class, derived from the catalogue.
   *
   * Kept because the solver still reports it for logs and badges, but it is no
   * longer a question the user answers — `memory` is.
   */
  speed: 'fast' | 'balanced' | 'accurate';
  /** 显存 class the plan's own weight files imply. */
  memory: MemoryBudget;
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
