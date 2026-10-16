import { useEffect, useState } from 'react';
import { Users } from 'lucide-react';
import { useTranslation } from 'react-i18next';

import { useStore } from '@/store/useStore';
import { GlassSwitch } from '@/components/ui/glass-switch';
import { hasInstalledDiarizationModel, listModels } from '@/lib/models';

/**
 * Batch speaker-separation control for the queue toolbar.
 *
 * Speaker separation is decided **per file** (see `FileItem`), so this switch is
 * only a convenience: it applies one value to every pending file at once. It
 * reads as "on" only when all pending files already agree.
 *
 * It is disabled until a diar-capable model is installed, because the solver
 * would otherwise have nothing to run the diarization with.
 */
export function BatchDiarizationToggle() {
  const { t } = useTranslation();
  const files = useStore((state) => state.files);
  const applyDiarizationToAll = useStore((state) => state.applyDiarizationToAll);

  const pendingFiles = files.filter((file) => file.status === 'pending');
  const pendingCount = pendingFiles.length;
  const allPendingDiarization =
    pendingCount > 0 && pendingFiles.every((file) => file.diarization);

  const [hasDiarizationModel, setHasDiarizationModel] = useState(false);
  useEffect(() => {
    let cancelled = false;
    listModels()
      .then((models) => {
        if (!cancelled) setHasDiarizationModel(hasInstalledDiarizationModel(models));
      })
      .catch(() => {
        if (!cancelled) setHasDiarizationModel(false);
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
      <Users className="w-3.5 h-3.5 text-text-tertiary dark:text-gray-500" />
      <span className="text-xs text-text-secondary dark:text-gray-400">
        {t('bottomBar.multiSpeaker')}
      </span>
      <GlassSwitch
        checked={allPendingDiarization}
        disabled={!hasDiarizationModel}
        onCheckedChange={(checked) => applyDiarizationToAll(checked)}
        aria-label={t('bottomBar.multiSpeaker')}
        className={`border border-divider dark:border-transparent ${
          hasDiarizationModel ? '' : 'opacity-60'
        }`}
      />
    </div>
  );
}
