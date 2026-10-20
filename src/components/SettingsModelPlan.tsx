import { useCallback, useEffect, useMemo, useState } from 'react';
import { Info, Loader2 } from 'lucide-react';
import { useTranslation } from 'react-i18next';

import { useStore } from '@/store/useStore';
import type { LanguageProfile, MemoryBudget } from '@/lib/modelPlan';
import { CustomSelect, type SelectOption } from '@/components/CustomSelect';
import { GlassButton } from '@/components/ui/glass-button';
import { GlassSwitch } from '@/components/ui/glass-switch';
import { GlassTooltip } from '@/components/ui/glass-tooltip';
import {
  downloadModel,
  formatModelSize,
  listModels,
  modelManifestSize,
  type ModelStatus,
} from '@/lib/models';
import { resolveModelPlan, type ModelPlan, type PlanResponse } from '@/lib/modelPlan';

/**
 * Settings ▸ Models ▸ "Recognition plan" (design §8.1).
 *
 * Four questions, and those four fully determine the plan:
 *
 *   1. 语种场景 — 普通话为主 / 中英混合 / 多语种
 *   2. 说话人分离
 *   3. 词级时间戳
 *   4. 显存占用 — 小 / 中 / 大
 *
 * Speed is deliberately absent: within this catalogue a model's weight size
 * decides its footprint *and* its throughput, so asking both would be one
 * question too many. Every manual override pins one step; the solver fills the
 * rest. A missing model is surfaced as a gap with a download action rather than
 * as a broken feature (design §5.6).
 */

const SLOT_ORDER = ['vad', 'asr', 'diar'] as const;
type Slot = (typeof SLOT_ORDER)[number];

const SLOT_LABEL_KEY: Record<Slot, string> = {
  vad: 'modelPlan.slotVad',
  asr: 'modelPlan.slotAsr',
  diar: 'modelPlan.slotDiar',
};

/**
 * Hover text explaining what each slot actually does.
 *
 * These must go through <GlassTooltip>, not a `title` attribute: the app runs in
 * a WKWebView, which does not render native tooltips — the hint was invisible.
 */
const SLOT_HINT_KEY: Record<Slot, string> = {
  vad: 'modelPlan.slotVadHint',
  asr: 'modelPlan.slotAsrHint',
  diar: 'modelPlan.slotDiarHint',
};

/** Roles a model must have to be selectable for a manual slot. */
const SLOT_ROLE: Record<Slot, string> = {
  asr: 'asr',
  vad: 'vad',
  diar: 'diar',
};

/** i18n key for a plan's human-readable title, falling back to the plan id. */
const planTitleKey = (plan: ModelPlan): string => plan.titleKey || 'modelPlan.effective';

export function SettingsModelPlan() {
  const { t } = useTranslation();
  const settings = useStore((state) => state.settings);
  const updateSettings = useStore((state) => state.updateSettings);
  const modelOverrides = useStore((state) => state.modelOverrides);
  const setModelOverride = useStore((state) => state.setModelOverride);

  const [models, setModels] = useState<ModelStatus[]>([]);
  const [plan, setPlan] = useState<PlanResponse | null>(null);
  const [isResolving, setIsResolving] = useState(false);
  const [resolveError, setResolveError] = useState('');
  const [showWhy, setShowWhy] = useState(false);
  const [downloadingId, setDownloadingId] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    listModels()
      .then((list) => {
        if (!cancelled) setModels(list);
      })
      .catch((error) => console.error('[SettingsModelPlan] list_models failed:', error));
    return () => {
      cancelled = true;
    };
  }, []);

  const refreshModels = useCallback(async () => {
    try {
      setModels(await listModels());
    } catch (error) {
      console.error('[SettingsModelPlan] list_models failed:', error);
    }
  }, []);

  // Live preview: re-solve whenever any requirement changes (design §8.2.1).
  useEffect(() => {
    let cancelled = false;
    setIsResolving(true);
    setResolveError('');

    resolveModelPlan({
      requireWordTimestamps: settings.requireWordTimestamps,
      enableDiarization: settings.defaultDiarization,
      language: settings.outputLanguage,
      languageProfile: settings.languageProfile,
      memoryBudget: settings.memoryBudget,
      // Overrides are applied to the preview so the user sees their pinning.
      // This must be sent: without it the preview showed the solver's unpinned
      // plan while the run executed the pinned one.
      modelOverrides,
    })
      .then((response) => {
        if (!cancelled) setPlan(response);
      })
      .catch((error) => {
        console.error('[SettingsModelPlan] resolve_model_plan failed:', error);
        if (!cancelled) setResolveError(String(error));
      })
      .finally(() => {
        if (!cancelled) setIsResolving(false);
      });

    return () => {
      cancelled = true;
    };
  }, [
    settings.requireWordTimestamps,
    settings.defaultDiarization,
    settings.languageProfile,
    settings.memoryBudget,
    settings.outputLanguage,
    modelOverrides,
  ]);

  const installedById = useMemo(() => {
    const map = new Map<string, ModelStatus>();
    for (const model of models) map.set(model.id, model);
    return map;
  }, [models]);

  const slotOptions = useMemo(() => {
    const build = (slot: Slot): SelectOption<string>[] => {
      const role = SLOT_ROLE[slot];
      const eligible = models.filter((model) =>
        model.roles.length > 0
          ? model.roles.includes(role)
          : role === 'asr',
      );
      return [
        { value: '', label: t('modelPlan.slotAuto') },
        ...eligible.map((model) => ({
          value: model.id,
          label: model.installed
            ? model.display_name
            : `${model.display_name} · ${t('modelPlan.notInstalled')}`,
        })),
      ];
    };

    // The VAD slot's "auto" is the bundled Silero VAD, which is shipped inside
    // the app rather than downloaded, so it is not in the catalogue and cannot
    // appear as a regular option. Name it in the label so "auto" is not
    // mysterious, and keep the catalogue's downloadable VADs below it.
    const vad = build('vad');
    vad[0] = { value: '', label: t('modelPlan.slotVadAuto') };

    return {
      asr: build('asr'),
      vad,
      diar: build('diar'),
    } satisfies Record<Slot, SelectOption<string>[]>;
  }, [models, t]);

  const handleDownload = useCallback(async (id: string) => {
    setDownloadingId(id);
    try {
      await downloadModel(id);
    } catch (error) {
      console.error('[SettingsModelPlan] download_model failed:', error);
    } finally {
      setDownloadingId(null);
      await refreshModels();
    }
  }, [refreshModels]);

  // Gaps whose model is known and not yet installed get a download action.
  const actionableGaps = useMemo(
    () =>
      (plan?.gaps ?? []).filter((gap) => {
        const id = gap.suggestedModelId;
        return Boolean(id) && installedById.get(id as string)?.installed !== true;
      }),
    [plan, installedById],
  );

  const effectivePlan = plan?.plan;

  return (
    <section className="space-y-4" aria-label={t('modelPlan.title')}>
      <div className="p-4 rounded-2xl border border-divider dark:border-zinc-700 bg-bg-secondary/70 dark:bg-zinc-900/50 space-y-4">
        <h3 className="text-xs font-semibold text-text-secondary dark:text-gray-400 uppercase tracking-wide">
          {t('modelPlan.title')}
        </h3>

        {/* The four questions (design §5.2 inputs). */}
        <div className="space-y-3">
          <div className="flex items-center justify-between gap-4">
            <div className="min-w-0">
              <span className="text-xs text-text-secondary dark:text-gray-300">{t('modelPlan.languageProfile')}</span>
              <p className="text-xxs text-text-tertiary dark:text-gray-500 mt-0.5">
                {t('modelPlan.languageProfileHint')}
              </p>
            </div>
            <CustomSelect<LanguageProfile>
              value={settings.languageProfile}
              onChange={(value) => updateSettings({ languageProfile: value })}
              testId="plan-language-profile"
              options={[
                { value: 'mandarin', label: t('modelPlan.languageMandarin') },
                { value: 'mixed', label: t('modelPlan.languageMixed') },
                { value: 'multilingual', label: t('modelPlan.languageMultilingual') },
              ]}
            />
          </div>

          <div className="flex items-center justify-between gap-4">
            <div className="min-w-0">
              <span className="text-xs text-text-secondary dark:text-gray-300">{t('modelPlan.diarization')}</span>
              <p className="text-xxs text-text-tertiary dark:text-gray-500 mt-0.5">
                {t('modelPlan.diarizationDefaultHint')}
              </p>
            </div>
            <GlassSwitch
              checked={settings.defaultDiarization}
              onCheckedChange={(checked) => updateSettings({ defaultDiarization: checked })}
              data-testid="plan-diarization"
              aria-label={t('modelPlan.diarization')}
            />
          </div>

          <div className="flex items-center justify-between gap-4">
            <div className="min-w-0">
              <span className="text-xs text-text-secondary dark:text-gray-300">{t('modelPlan.wordTimestamps')}</span>
              <p className="text-xxs text-text-tertiary dark:text-gray-500 mt-0.5">
                {t('modelPlan.wordTimestampsHint')}
              </p>
            </div>
            <GlassSwitch
              checked={settings.requireWordTimestamps}
              onCheckedChange={(checked) => updateSettings({ requireWordTimestamps: checked })}
              data-testid="plan-word-timestamps"
              aria-label={t('modelPlan.wordTimestamps')}
            />
          </div>

          <div className="flex items-center justify-between gap-4">
            <div className="min-w-0">
              <span className="text-xs text-text-secondary dark:text-gray-300">{t('modelPlan.memoryBudget')}</span>
              <p className="text-xxs text-text-tertiary dark:text-gray-500 mt-0.5">
                {t('modelPlan.memoryBudgetHint')}
              </p>
            </div>
            <CustomSelect<MemoryBudget>
              value={settings.memoryBudget}
              onChange={(value) => updateSettings({ memoryBudget: value })}
              testId="plan-memory-budget"
              options={[
                { value: 'small', label: t('modelPlan.memorySmall') },
                { value: 'medium', label: t('modelPlan.memoryMedium') },
                { value: 'large', label: t('modelPlan.memoryLarge') },
              ]}
            />
          </div>
        </div>

        {/* Live preview of the resolved plan */}
        <div
          data-testid="plan-preview"
          // The resolved plan as machine-readable attributes: resolution is
          // async, so a scenario (or a support report) needs a way to wait for
          // "the answer is in" rather than polling the rendered text.
          data-plan-id={effectivePlan?.id ?? ''}
          data-plan-models={effectivePlan?.modelIds.join(',') ?? ''}
          data-plan-memory={effectivePlan?.cost.memory ?? ''}
          data-plan-timestamps={effectivePlan?.capabilities.timestamps ?? ''}
          data-plan-resolving={isResolving ? 'true' : 'false'}
          className="p-3 rounded-2xl border border-divider dark:border-zinc-700 bg-bg-secondary dark:bg-zinc-800/50 space-y-2"
        >
          <div className="flex items-center gap-2">
            <span className="text-xxs uppercase tracking-wide text-text-tertiary dark:text-gray-500">
              {t('modelPlan.effective')}
            </span>
            {isResolving && <Loader2 className="w-3 h-3 animate-spin text-text-tertiary dark:text-gray-500" />}
          </div>

          {resolveError ? (
            <p className="text-xs text-danger">{resolveError}</p>
          ) : effectivePlan ? (
            <>
              <p className="text-sm font-semibold text-text-primary dark:text-white">
                {t(planTitleKey(effectivePlan))}
              </p>
              <p className="text-xxs text-text-tertiary dark:text-gray-500">
                {effectivePlan.modelIds.join(' + ') || '—'}
              </p>
              {/*
                The footprint class comes from the chosen models' own weight
                files, so this line and the download size can never disagree.
                The timing grade is read from `capabilities.timestamps` rather
                than from the plan title, because the same shape can reach
                segment or word timing depending on the ASR that fills it —
                "VAD + ASR" is estimated timing with SenseVoice and measured
                word timing with qwen3_asr.
              */}
              <p className="text-xxs text-text-tertiary dark:text-gray-600">
                {t('modelPlan.planFootprint', {
                  memory: t(
                    {
                      small: 'modelPlan.memorySmall',
                      medium: 'modelPlan.memoryMedium',
                      large: 'modelPlan.memoryLarge',
                    }[effectivePlan.cost.memory],
                  ),
                })}
                {' · '}
                {t(
                  {
                    word: 'modelPlan.timestampWord',
                    segment: 'modelPlan.timestampSegment',
                    turns: 'modelPlan.timestampTurns',
                    none: 'modelPlan.timestampEstimate',
                  }[effectivePlan.capabilities.timestamps],
                )}
              </p>
              {(plan?.candidates.length ?? 0) > 1 && (
                <button
                  type="button"
                  onClick={() => setShowWhy((value) => !value)}
                  className="flex items-center gap-1 text-xxs text-primary hover:underline"
                >
                  <Info className="w-3 h-3" />
                  {t('modelPlan.why')}
                </button>
              )}
              {showWhy && (
                <ol className="mt-2 space-y-1">
                  {plan?.candidates.map((candidate) => (
                    <li key={candidate.id} className="text-xxs text-text-secondary dark:text-gray-400">
                      {t(planTitleKey(candidate))}
                      {candidate.modelIds.length > 0 && (
                        <span className="text-text-tertiary dark:text-gray-500">
                          {' — '}
                          {candidate.modelIds.join(' + ')}
                        </span>
                      )}
                    </li>
                  ))}
                </ol>
              )}
            </>
          ) : null}
        </div>

        {/* Gaps: explain the missing capability and offer the download. */}
        {actionableGaps.length > 0 && (
          <div className="p-3 rounded-2xl border border-warning/40 dark:border-warning/40 bg-warning/5 space-y-2">
            <p className="text-xs font-semibold text-warning">{t('modelPlan.gapTitle')}</p>
            {actionableGaps.map((gap) => {
              const id = gap.suggestedModelId as string;
              const model = installedById.get(id);
              return (
                <div key={`${gap.requirement}:${id}`} className="flex items-center justify-between gap-3">
                  <span className="text-xxs text-text-secondary dark:text-gray-400">
                    {t('modelPlan.gapSuggest', {
                      model: model?.display_name ?? id,
                      size: model ? formatModelSize(modelManifestSize(model)) : '—',
                    })}
                  </span>
                  <GlassButton
                    variant="outline"
                    size="sm"
                    disabled={downloadingId === id}
                    onClick={() => void handleDownload(id)}
                  >
                    {downloadingId === id ? (
                      <Loader2 className="w-3 h-3 animate-spin" />
                    ) : null}
                    {t('modelPlan.download')}
                  </GlassButton>
                </div>
              );
            })}
          </div>
        )}

        {/* Advanced: pin a specific model per step (design §8.2.3). */}
        <details className="group">
          <summary className="cursor-pointer text-xs font-semibold text-text-secondary dark:text-gray-400 uppercase tracking-wide">
            {t('modelPlan.advanced')}
          </summary>
          <div className="mt-3 space-y-3">
            {SLOT_ORDER.map((slot) => (
              <div key={slot} className="flex items-center justify-between gap-4">
                <GlassTooltip content={t(SLOT_HINT_KEY[slot])} side="top">
                  <span className="text-xs text-text-secondary dark:text-gray-300 underline decoration-dotted decoration-text-tertiary/60 underline-offset-2">
                    {t(SLOT_LABEL_KEY[slot])}
                  </span>
                </GlassTooltip>
                <CustomSelect
                  value={modelOverrides[slot] ?? ''}
                  options={slotOptions[slot]}
                  onChange={(value) => setModelOverride(slot, value)}
                />
              </div>
            ))}
          </div>
        </details>
      </div>
    </section>
  );
}
