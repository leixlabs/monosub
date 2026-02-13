import { Folder, FolderOpen, StopCircle, HardDrive } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { motion } from 'framer-motion';
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
  const setCurrentTaskId = useStore((state) => state.setCurrentTaskId);
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
      setCurrentTaskId(null);
    } catch (error) {
      console.error('[cancel_transcription] Failed:', error);
    }
  };

  return (
    <div className="bg-bg-secondary dark:bg-zinc-950 h-14 px-6 flex items-center justify-between border-t border-divider/40 dark:border-zinc-800/40 transition-colors">
      <div className="flex items-center gap-2">
        <HardDrive className="w-4 h-4 text-text-tertiary dark:text-gray-500" />
        <span className="text-xs text-text-tertiary dark:text-gray-500 mr-2">{t('bottomBar.saveLocation')}</span>

        <div className="flex bg-bg-secondary dark:bg-zinc-800 rounded-lg p-0.5">
          <motion.button
            whileTap={{ scale: 0.97 }}
            onClick={() => setStorageMode('same')}
            className={`flex items-center gap-1.5 px-3 py-1.5 text-xs rounded-md transition-all ${storageMode === 'same'
                ? 'bg-white dark:bg-zinc-700 text-text-primary dark:text-white shadow-apple'
                : 'text-text-secondary dark:text-gray-400 hover:text-text-primary dark:hover:text-white'
              }`}
          >
            <Folder className="w-3.5 h-3.5" />
            {t('bottomBar.sameDirectory')}
          </motion.button>
          <motion.button
            whileTap={{ scale: 0.97 }}
            onClick={handleSelectCustomPath}
            className={`flex items-center gap-1.5 px-3 py-1.5 text-xs rounded-md transition-all ${storageMode === 'custom'
                ? 'bg-white dark:bg-zinc-700 text-text-primary dark:text-white shadow-apple'
                : 'text-text-secondary dark:text-gray-400 hover:text-text-primary dark:hover:text-white'
              }`}
          >
            <FolderOpen className="w-3.5 h-3.5" />
            {storageMode === 'custom' && customStoragePath
              ? truncatePath(customStoragePath)
              : t('bottomBar.selectDirectory')}
          </motion.button>
        </div>

        <span className="mx-3 w-px h-4 bg-divider dark:bg-zinc-700" />

        <span className="text-xs text-text-tertiary dark:text-gray-500">
          {t('bottomBar.format')} <span className="text-text-secondary dark:text-gray-300 font-medium">{settings.outputFormat.toUpperCase()}</span>
        </span>
        <span className="mx-2 w-px h-3 bg-divider dark:bg-zinc-700" />
        <span className="text-xs text-text-tertiary dark:text-gray-500">
          {t('bottomBar.language')} <span className="text-text-secondary dark:text-gray-300 font-medium">{getLanguageLabel()}</span>
        </span>
      </div>

      <div className="flex items-center gap-3">
        {isProcessing && (
          <motion.button
            whileTap={{ scale: 0.97 }}
            onClick={handleStop}
            className="flex items-center gap-1.5 px-4 py-1.5 bg-danger/10 dark:bg-danger/20 text-danger text-xs font-medium rounded-lg hover:bg-danger/20 dark:hover:bg-danger/30 transition-colors"
          >
            <StopCircle className="w-3.5 h-3.5" />
            {t('bottomBar.stop')}
          </motion.button>
        )}
      </div>
    </div>
  );
}
