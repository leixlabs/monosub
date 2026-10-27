import { useEffect, useState } from 'react';
import { ClipboardPaste, FolderOpen, Link2 } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { open } from '@tauri-apps/plugin-dialog';
import { useStore } from '@/store/useStore';
import { GlassButton } from '@/components/ui/glass-button';
import { GlassSwitch } from '@/components/ui/glass-switch';
import { loadDefaultUrlImportDir } from '@/lib/urlImport';

/**
 * Settings → URL import (url-import-design §5.4, §7-2).
 *
 * Where an import's media and subtitles are written, whether to watch the
 * clipboard, and whether to always recognise locally instead of taking the
 * platform subtitle. There is deliberately no cookie setting: asking users to
 * export a cookie jar is too much to ask (see §4.5).
 */
export function SettingsUrlImport() {
  const { t } = useTranslation();
  const settings = useStore((state) => state.settings);
  const updateSettings = useStore((state) => state.updateSettings);
  const [defaultDir, setDefaultDir] = useState('');

  useEffect(() => {
    let cancelled = false;
    void loadDefaultUrlImportDir()
      .then((dir) => {
        if (!cancelled) setDefaultDir(dir);
      })
      .catch((error) => {
        console.warn('[SettingsUrlImport] could not resolve the default directory', error);
      });
    return () => {
      cancelled = true;
    };
  }, []);

  const handleChooseDir = async () => {
    try {
      const selected = await open({ directory: true, multiple: false });
      if (typeof selected === 'string') {
        updateSettings({ urlImportOutputDir: selected });
      }
    } catch (error) {
      console.error('[SettingsUrlImport] could not choose a directory', error);
    }
  };

  const outputDir = settings.urlImportOutputDir.trim();

  return (
    <section className="space-y-4">
      {/* Download / save directory */}
      <div className="p-4 rounded-2xl border border-divider dark:border-zinc-700 bg-bg-secondary/70 dark:bg-zinc-900/50 space-y-3">
        <div>
          <p className="text-sm font-semibold text-text-primary dark:text-gray-200">
            {t('urlImport.settings.outputDir')}
          </p>
          <p className="mt-0.5 text-xs text-text-secondary dark:text-gray-400 leading-relaxed">
            {t('urlImport.settings.outputDirHint')}
          </p>
        </div>
        <div className="flex items-center gap-2">
          <code
            className="flex-1 min-w-0 truncate rounded-lg border border-divider dark:border-zinc-700 bg-bg-secondary dark:bg-zinc-800/60 px-2.5 py-1.5 text-xs text-text-primary dark:text-gray-300"
            title={outputDir || defaultDir}
            data-testid="url-import-output-dir"
          >
            {outputDir || defaultDir || '…'}
          </code>
          <GlassButton variant="outline" size="sm" onClick={() => void handleChooseDir()}>
            <FolderOpen className="w-4 h-4" />
            {t('urlImport.settings.chooseFolder')}
          </GlassButton>
          {outputDir && (
            <GlassButton
              variant="ghost"
              size="sm"
              onClick={() => updateSettings({ urlImportOutputDir: '' })}
            >
              {t('urlImport.outputDirDefault')}
            </GlassButton>
          )}
        </div>
      </div>

      {/* Clipboard watch */}
      <div className="p-4 rounded-2xl border border-divider dark:border-zinc-700 bg-bg-secondary/70 dark:bg-zinc-900/50">
        <div className="flex items-center justify-between gap-4">
          <div className="min-w-0">
            <p className="flex items-center gap-1.5 text-sm font-semibold text-text-primary dark:text-gray-200">
              <ClipboardPaste className="w-3.5 h-3.5" />
              {t('urlImport.settings.clipboardWatch')}
            </p>
            <p className="mt-0.5 text-xs text-text-secondary dark:text-gray-400 leading-relaxed">
              {t('urlImport.settings.clipboardWatchHint')}
            </p>
          </div>
          <GlassSwitch
            checked={settings.clipboardWatchEnabled}
            onCheckedChange={(checked) => updateSettings({ clipboardWatchEnabled: checked })}
            data-testid="url-import-clipboard-watch"
          />
        </div>
      </div>

      {/* Prefer local recognition */}
      <div className="p-4 rounded-2xl border border-divider dark:border-zinc-700 bg-bg-secondary/70 dark:bg-zinc-900/50">
        <div className="flex items-center justify-between gap-4">
          <div className="min-w-0">
            <p className="flex items-center gap-1.5 text-sm font-semibold text-text-primary dark:text-gray-200">
              <Link2 className="w-3.5 h-3.5" />
              {t('urlImport.settings.preferLocal')}
            </p>
            <p className="mt-0.5 text-xs text-text-secondary dark:text-gray-400 leading-relaxed">
              {t('urlImport.settings.preferLocalHint')}
            </p>
          </div>
          <GlassSwitch
            checked={settings.preferLocalRecognition}
            onCheckedChange={(checked) => updateSettings({ preferLocalRecognition: checked })}
            data-testid="url-import-prefer-local"
          />
        </div>
      </div>
    </section>
  );
}
