import { useEffect, useState } from 'react';
import { Languages } from 'lucide-react';
import { useTranslation } from 'react-i18next';

import { useStore } from '@/store/useStore';
import { GlassSwitch } from '@/components/ui/glass-switch';
import { GlassTooltip } from '@/components/ui/glass-tooltip';
import { hasInstalledRole, listModels, TRANSLATE_ROLE } from '@/lib/models';

/**
 * Batch translation control for the queue toolbar.
 *
 * The sibling of `BatchDiarizationToggle`, and for the same reason: whether a
 * run also writes a translated copy is decided **per file** (see `FileItem`),
 * and this switch is the convenience that applies one value to every pending
 * file at once. It reads as "on" only when all pending files already agree.
 *
 * `settings.translateEnabled` is only the *default* a newly queued file starts
 * from — the actual per-run decision lives here and on the row. The target
 * language and the model stay in Settings: those are configure-once, and asking
 * them per run would be the same question over and over.
 *
 * It is disabled until a translation model is installed, because the backend
 * would otherwise reject the run with `MODEL_NOT_INSTALLED:<id>`.
 */
export function BatchTranslationToggle() {
  const { t } = useTranslation();
  const files = useStore((state) => state.files);
  const applyTranslateToAll = useStore((state) => state.applyTranslateToAll);

  const pendingFiles = files.filter((file) => file.status === 'pending');
  const pendingCount = pendingFiles.length;
  const allPendingTranslate = pendingCount > 0 && pendingFiles.every((file) => file.translate);

  const [hasTranslationModel, setHasTranslationModel] = useState(false);
  useEffect(() => {
    let cancelled = false;
    listModels()
      .then((models) => {
        if (!cancelled) setHasTranslationModel(hasInstalledRole(models, TRANSLATE_ROLE));
      })
      .catch(() => {
        if (!cancelled) setHasTranslationModel(false);
      });
    return () => {
      cancelled = true;
    };
  }, []);

  if (pendingCount === 0) {
    return null;
  }

  return (
    <div className="flex items-center gap-2">
      <Languages className="w-3.5 h-3.5 text-text-tertiary dark:text-gray-500" />
      <GlassTooltip
        content={
          hasTranslationModel
            ? t('bottomBar.translateHint')
            : t('settings.translate.noModels')
        }
        side="top"
      >
        <span className="text-xs text-text-secondary dark:text-gray-400">
          {t('bottomBar.translate')}
        </span>
      </GlassTooltip>
      <GlassSwitch
        checked={allPendingTranslate}
        disabled={!hasTranslationModel}
        onCheckedChange={(checked) => applyTranslateToAll(checked)}
        aria-label={t('bottomBar.translate')}
        data-testid="batch-translation"
        className={`border border-divider dark:border-transparent ${
          hasTranslationModel ? '' : 'opacity-60'
        }`}
      />
    </div>
  );
}
