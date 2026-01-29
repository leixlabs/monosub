import { Folder, FolderOpen, StopCircle, HardDrive } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { useStore } from '@/store/useStore';
import { open } from '@tauri-apps/plugin-dialog';
import { invoke } from '@tauri-apps/api/core';

export function BottomBar() {
  const { t } = useTranslation();
  const storageMode = useStore((state) => state.storageMode);
  const setStorageMode = useStore((state) => state.setStorageMode);
  const customStoragePath = useStore((state) => state.customStoragePath);
  const setCustomStoragePath = useStore((state) => state.setCustomStoragePath);
  const isProcessing = useStore((state) => state.isProcessing);
  const setIsProcessing = useStore((state) => state.setIsProcessing);
  const files = useStore((state) => state.files);
  const updateFile = useStore((state) => state.updateFile);
  const settings = useStore((state) => state.settings);

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

  const handleStop = async () => {
    const processingFile = files.find(f => f.status === 'processing');
    if (!processingFile) return;

    try {
      await invoke('cancel_transcription', { taskId: processingFile.id });
      updateFile(processingFile.id, { status: 'pending', progress: 0 });
      setIsProcessing(false);
    } catch (error) {
      console.error('[cancel_transcription] Failed:', error);
    }
  };

  return (
    <div className="bg-bg-card/80 backdrop-blur-lg border-t border-divider h-14 px-6 flex items-center justify-between">
      <div className="flex items-center gap-2">
        <HardDrive className="w-4 h-4 text-text-tertiary" />
        <span className="text-xs text-text-tertiary mr-2">{t('bottomBar.saveLocation')}</span>

        <div className="flex bg-bg-secondary rounded-lg p-0.5">
          <button
            onClick={() => setStorageMode('same')}
            className={`flex items-center gap-1.5 px-3 py-1.5 text-xs rounded-md transition-all ${
              storageMode === 'same'
                ? 'bg-white text-text-primary shadow-apple'
                : 'text-text-secondary hover:text-text-primary'
            }`}
          >
            <Folder className="w-3.5 h-3.5" />
            {t('bottomBar.sameDirectory')}
          </button>
          <button
            onClick={handleSelectCustomPath}
            className={`flex items-center gap-1.5 px-3 py-1.5 text-xs rounded-md transition-all ${
              storageMode === 'custom'
                ? 'bg-white text-text-primary shadow-apple'
                : 'text-text-secondary hover:text-text-primary'
            }`}
          >
            <FolderOpen className="w-3.5 h-3.5" />
            {storageMode === 'custom' && customStoragePath
              ? truncatePath(customStoragePath)
              : t('bottomBar.selectDirectory')}
          </button>
        </div>

        <span className="mx-3 w-px h-4 bg-divider" />

        <span className="text-xs text-text-tertiary">
          {t('bottomBar.format')} <span className="text-text-secondary font-medium">{settings.outputFormat.toUpperCase()}</span>
        </span>
        <span className="mx-2 w-px h-3 bg-divider" />
        <span className="text-xs text-text-tertiary">
          {t('bottomBar.language')} <span className="text-text-secondary font-medium">{getLanguageLabel()}</span>
        </span>
      </div>

      <div className="flex items-center gap-3">
        {isProcessing && (
          <button
            onClick={handleStop}
            className="flex items-center gap-1.5 px-4 py-1.5 bg-danger/10 text-danger text-xs font-medium rounded-lg hover:bg-danger/20 transition-colors"
          >
            <StopCircle className="w-3.5 h-3.5" />
            {t('bottomBar.stop')}
          </button>
        )}
      </div>
    </div>
  );
}
