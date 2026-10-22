import { useCallback, useEffect, useState } from 'react';
import { FolderOpen, HardDrive, Loader2, RotateCcw } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { open } from '@tauri-apps/plugin-dialog';

import { GlassButton } from '@/components/ui/glass-button';
import { reportFrontendError } from '@/lib/logging';
import {
  getModelsDir,
  openModelsDir,
  setModelsDir,
  type ModelsDirInfo,
} from '@/lib/models';

/**
 * Models ▸ "Storage location" (Settings → 模型).
 *
 * Models are 0.2–1.2 GB each, so re-downloading them after a reinstall or on a
 * second machine is a real cost. This lets the user point the app at a folder
 * that already contains them.
 *
 * Changing the location does **not** move existing files — the whole point is to
 * reuse models that are already on disk. Rust validates the directory before
 * persisting it, so a bad choice fails here rather than on the next download.
 */
export function SettingsModelsDir({ onChanged }: { onChanged: () => void }) {
  const { t } = useTranslation();
  const [info, setInfo] = useState<ModelsDirInfo | null>(null);
  const [isBusy, setIsBusy] = useState(false);
  const [error, setError] = useState('');

  const load = useCallback(async () => {
    try {
      setInfo(await getModelsDir());
      setError('');
    } catch (err) {
      console.error('[SettingsModelsDir] get_models_dir failed:', err);
      setError(String(err));
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  const apply = useCallback(
    async (path: string | null) => {
      setIsBusy(true);
      setError('');
      try {
        setInfo(await setModelsDir(path));
        // Install state depends on the directory, so the model list must reload.
        onChanged();
      } catch (err) {
        console.error('[SettingsModelsDir] set_models_dir failed:', err);
        setError(String(err));
      } finally {
        setIsBusy(false);
      }
    },
    [onChanged],
  );

  const handleChoose = useCallback(async () => {
    try {
      const selected = await open({ directory: true, defaultPath: info?.path });
      if (typeof selected === 'string' && selected.length > 0) {
        await apply(selected);
      }
    } catch (err) {
      console.error('[SettingsModelsDir] directory picker failed:', err);
      setError(String(err));
    }
  }, [apply, info?.path]);

  /**
   * "Open the models folder" — reported, not swallowed.
   *
   * This used to be `void openModelsDir()`, so a failure (Rust could not launch
   * the file manager, or it exited non-zero — which the old `spawn()` never
   * checked) produced no feedback at all: on macOS the button simply appeared
   * dead. The error is shown in the card and written to the log, because in a
   * release build the WebView console does not exist.
   */
  const handleOpen = useCallback(async () => {
    setError('');
    try {
      await openModelsDir();
    } catch (err) {
      console.error('[open_models_dir] Failed:', err);
      reportFrontendError(err, { context: 'open_models_dir' });
      setError(String(err));
    }
  }, []);

  const displayPath = info?.path ?? '';

  return (
    <div className="p-4 rounded-2xl border border-divider dark:border-zinc-700 bg-bg-secondary/70 dark:bg-zinc-900/50 space-y-3">
      <div className="flex items-start justify-between gap-4">
        <div className="min-w-0">
          <div className="flex items-center gap-2">
            <HardDrive className="w-4 h-4 text-text-primary dark:text-gray-300" />
            <span className="text-sm font-semibold text-text-primary dark:text-gray-200">
              {t('models.storageLocation')}
            </span>
            {info?.isCustom && (
              <span className="text-xxs px-1.5 py-0.5 rounded-full bg-primary/15 text-primary font-medium">
                {t('models.custom')}
              </span>
            )}
            {isBusy && <Loader2 className="w-3.5 h-3.5 animate-spin text-text-tertiary" />}
          </div>
          <p className="text-xs text-text-secondary dark:text-gray-400 mt-1 break-all" title={displayPath}>
            {displayPath || '—'}
          </p>
          <p className="text-xxs text-text-tertiary dark:text-gray-500 mt-1">
            {info?.source === 'env'
              ? t('models.pathFromEnv')
              : t('models.storageLocationHint')}
          </p>
        </div>
      </div>

      {error && <p className="text-xs text-danger break-all">{error}</p>}

      {info?.source === 'env' ? (
        // An env override wins over the setting, so changing the setting here
        // would appear to do nothing. Say so instead of offering a dead control.
        <p className="text-xxs text-warning">{t('models.pathEnvOverrides')}</p>
      ) : (
        <div className="flex flex-wrap items-center gap-2">
          <GlassButton variant="outline" size="sm" onClick={() => void handleChoose()} disabled={isBusy}>
            <FolderOpen className="w-4 h-4" />
            {t('models.changeLocation')}
          </GlassButton>

          {info?.isCustom && (
            <GlassButton variant="ghost" size="sm" onClick={() => void apply(null)} disabled={isBusy}>
              <RotateCcw className="w-4 h-4" />
              {t('models.resetLocation')}
            </GlassButton>
          )}

          <GlassButton variant="ghost" size="sm" onClick={() => void handleOpen()} disabled={isBusy}>
            <FolderOpen className="w-4 h-4" />
            {t('models.openFolder')}
          </GlassButton>
        </div>
      )}
    </div>
  );
}
