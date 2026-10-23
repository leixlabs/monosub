import { Folder, FolderOpen, HardDrive, Languages } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { useStore } from '@/store/useStore';
import { open } from '@tauri-apps/plugin-dialog';
import { GlassButton } from '@/components/ui/glass-button';

export function BottomBar() {
  const { t } = useTranslation();
  const storageMode = useStore((state) => state.storageMode);
  const setStorageMode = useStore((state) => state.setStorageMode);
  const customStoragePath = useStore((state) => state.customStoragePath);
  const setCustomStoragePath = useStore((state) => state.setCustomStoragePath);
  const settings = useStore((state) => state.settings);
  const files = useStore((state) => state.files);
  // Whether *anything in the queue* will be translated. The per-file switch is
  // the decision; this is just its summary.
  const anyTranslateQueued = files.some(
    (file) => file.translate && (file.status === 'pending' || file.status === 'processing'),
  );

  const handleSelectCustomPath = async () => {
    try {
      const selected = await open({
        directory: true,
      });

      if (selected && typeof selected === 'string') {
        setCustomStoragePath(selected);
        setStorageMode('custom');
      }
    } catch (error) {
      console.error('Failed to select directory:', error);
    }
  };

  const truncatePath = (path: string, maxLength: number = 20) => {
    if (path.length <= maxLength) return path;
    const parts = path.split('/');
    const last = parts[parts.length - 1];
    if (last.length > maxLength) {
      return '...' + last.slice(-maxLength);
    }
    return '.../' + last;
  };

  const getLanguageLabel = () => {
    const labels: Record<string, string> = {
      'auto': t('bottomBar.auto'),
      'en': 'EN',
      'zh': '中文',
      'es': 'ES',
      'ja': '日本語',
      'de': 'DE',
      'fr': 'FR',
      'pt': 'PT',
      'zh-tw': '繁中',
      'ko': 'KR',
    };
    return labels[settings.outputLanguage] || settings.outputLanguage;
  };

  // The target language is a separate language from the recognition one, so it
  // gets its own chip; `getLanguageLabel` only reads `outputLanguage`.
  const getTargetLanguageLabel = () => {
    const labels: Record<string, string> = {
      'en': 'EN',
      'zh': '中文',
      'es': 'ES',
      'ja': '日本語',
      'de': 'DE',
      'fr': 'FR',
      'pt': 'PT',
      'zh-tw': '繁中',
      'ko': 'KR',
    };
    return labels[settings.translateTargetLanguage] || settings.translateTargetLanguage;
  };

  return (
    <div className="bg-bg-secondary dark:bg-zinc-950 h-14 px-6 flex-shrink-0 sticky bottom-0 z-30 flex items-center justify-between border-t border-divider/40 dark:border-zinc-800/40 transition-colors">
      <div className="flex items-center gap-2">
        <HardDrive className="w-4 h-4 text-text-tertiary dark:text-gray-500" />
        <span className="text-xs text-text-tertiary dark:text-gray-500 mr-2">{t('bottomBar.saveLocation')}</span>

        {/* 存储模式分段控件 - muted 变体支持亮暗双模式 */}
        <div className="flex bg-bg-tertiary dark:bg-zinc-800 rounded-lg p-0.5">
          <GlassButton
            variant="muted"
            size="sm"
            onClick={() => setStorageMode('same')}
            className={`h-7 px-2.5 text-xs ${storageMode === 'same'
                ? 'bg-white dark:bg-zinc-700 text-text-primary dark:text-white shadow-apple hover:scale-100'
                : ''
              }`}
          >
            <Folder className="w-3.5 h-3.5" />
            {t('bottomBar.sameDirectory')}
          </GlassButton>
          <GlassButton
            variant="muted"
            size="sm"
            onClick={() => void handleSelectCustomPath()}
            className={`h-7 px-2.5 text-xs ${storageMode === 'custom'
                ? 'bg-white dark:bg-zinc-700 text-text-primary dark:text-white shadow-apple hover:scale-100'
                : ''
              }`}
          >
            <FolderOpen className="w-3.5 h-3.5" />
            {storageMode === 'custom' && customStoragePath
              ? truncatePath(customStoragePath)
              : t('bottomBar.selectDirectory')}
          </GlassButton>
        </div>

        <span className="mx-3 w-px h-4 bg-divider dark:bg-zinc-700" />

        <span className="text-xs text-text-tertiary dark:text-gray-500">
          {t('bottomBar.format')} <span className="text-text-secondary dark:text-gray-300 font-medium">{settings.outputFormat.toUpperCase()}</span>
        </span>
        <span className="mx-2 w-px h-3 bg-divider dark:bg-zinc-700" />
        <span className="text-xs text-text-tertiary dark:text-gray-500">
          {t('bottomBar.language')} <span className="text-text-secondary dark:text-gray-300 font-medium">{getLanguageLabel()}</span>
        </span>
        {/* Only shown while at least one queued file will be translated: the
            chip is the visible signal that a `<stem>.<lang>.<ext>` file will
            also be written. It reads the QUEUE, not `settings.translateEnabled`
            — that field is only the default a new file starts from, and showing
            the chip while every row's switch is off would promise a file nobody
            is going to write. */}
        {anyTranslateQueued && (
          <>
            <span className="mx-2 w-px h-3 bg-divider dark:bg-zinc-700" />
            <span className="text-xs text-text-tertiary dark:text-gray-500 flex items-center gap-1">
              <Languages className="w-3 h-3" />
              {t('bottomBar.translationTarget')}{' '}
              <span className="text-text-secondary dark:text-gray-300 font-medium">
                {getTargetLanguageLabel()}
              </span>
            </span>
          </>
        )}
      </div>
    </div>
  );
}
