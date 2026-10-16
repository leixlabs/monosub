import { useCallback, useEffect, useMemo, useState } from 'react';
import { Info, Loader2 } from 'lucide-react';
import { useTranslation } from 'react-i18next';

import { useStore } from '@/store/useStore';
import { CustomSelect, type SelectOption } from '@/components/CustomSelect';
import { GlassButton } from '@/components/ui/glass-button';
import { GlassSwitch } from '@/components/ui/glass-switch';
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
 * Requirements-driven: the user states needs (word timestamps, speaker
 * separation, speed), and the solver returns a plan
 * which is previewed live. Every manual override pins one step; the solver
 * fills the rest. A missing model is surfaced as a gap with a download action
 * rather than as a broken feature (design §5.6).
 */

const SLOT_ORDER = ['vad', 'asr', 'diar'] as const;
type Slot = (typeof SLOT_ORDER)[number];

const SLOT_LABEL_KEY: Record<Slot, string> = {
  vad: 'modelPlan.slotVad',
  asr: 'modelPlan.slotAsr',
  diar: 'modelPlan.slotDiar',
};

/** Hover text explaining what each slot actually does. */
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
      speedPreference: settings.speedPreference,
      // Overrides are applied to the preview so the user sees their pinning.
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
    settings.speedPreference,
    settings.outputLanguage,
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
    return {
      asr: build('asr'),
      vad: build('vad'),
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

        {/* Requirements */}
        <div className="space-y-3">
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
              aria-label={t('modelPlan.wordTimestamps')}
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
              aria-label={t('modelPlan.diarization')}
            />
          </div>


          <div className="flex items-center justify-between gap-4">
            <span className="text-xs text-text-secondary dark:text-gray-300">{t('modelPlan.speed')}</span>
            <CustomSelect
              value={settings.speedPreference}
              onChange={(value) => updateSettings({ speedPreference: value })}
              options={[
                { value: 'fast', label: t('modelPlan.speedFast') },
                { value: 'balanced', label: t('modelPlan.speedBalanced') },
                { value: 'accurate', label: t('modelPlan.speedAccurate') },
              ]}
            />
          </div>
        </div>

        {/* Live preview of the resolved plan */}
        <div className="p-3 rounded-2xl border border-divider dark:border-zinc-700 bg-bg-secondary dark:bg-zinc-800/50 space-y-2">
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
                <span
                  className="text-xs text-text-secondary dark:text-gray-300 cursor-help underline decoration-dotted decoration-text-tertiary/60 underline-offset-2"
                  title={t(SLOT_HINT_KEY[slot])}
                >
                  {t(SLOT_LABEL_KEY[slot])}
                </span>
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
