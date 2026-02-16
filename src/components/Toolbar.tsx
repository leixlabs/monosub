import { Plus, Trash2, Film } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { motion } from 'framer-motion';
import { useStore } from '@/store/useStore';
import { open } from '@tauri-apps/plugin-dialog';
import { addPathsToQueue, MEDIA_EXTENSIONS } from '@/lib/fileImport';

export function Toolbar() {
  const { t } = useTranslation();
  const files = useStore((state) => state.files);
  const addFiles = useStore((state) => state.addFiles);
  const clearFiles = useStore((state) => state.clearFiles);
  const isProcessing = useStore((state) => state.isProcessing);

  const pendingCount = files.filter(f => f.status === 'pending').length;
  const processingCount = files.filter(f => f.status === 'processing').length;
  const completedCount = files.filter(f => f.status === 'completed').length;

  const handleAddFiles = async () => {
    try {
      const selected = await open({
        multiple: true,
        filters: [{
          name: 'Media',
          extensions: MEDIA_EXTENSIONS
        }]
      });

      if (!selected) return;

      const paths = Array.isArray(selected) ? selected : [selected];
      const result = await addPathsToQueue({
        paths,
        existingFiles: files,
        addFiles,
        source: 'Toolbar',
      });

      if (result.skippedDuplicates > 0) {
        console.warn(`[Toolbar] Skipped ${result.skippedDuplicates} duplicate file(s)`);
      }
      if (result.skippedUnsupported > 0) {
        console.warn(`[Toolbar] Skipped ${result.skippedUnsupported} unsupported file(s)`);
      }
    } catch (error) {
      console.error('Failed to select files:', error);
    }
  };

  return (
    <div className="bg-bg-secondary dark:bg-zinc-950 h-[64px] px-6 flex items-center justify-between transition-colors">
      <div className="flex items-center gap-4">
        <motion.button
          whileTap={{ scale: 0.97 }}
          onClick={handleAddFiles}
          className="flex items-center gap-2 px-4 py-2 bg-primary text-white text-sm font-medium rounded-xl hover:bg-primary-hover transition-colors shadow-glass-md"
        >
          <Plus className="w-4 h-4" />
          {t('toolbar.addFiles')}
        </motion.button>

        {files.length > 0 && (
          <div className="flex items-center gap-3 pl-4 border-l border-divider dark:border-zinc-700">
            <div className="flex items-center gap-1.5 text-xs text-text-secondary dark:text-gray-400">
              <Film className="w-3.5 h-3.5" />
              <span>{t('toolbar.filesCount', { count: files.length })}</span>
            </div>
            {pendingCount > 0 && (
              <span className="px-2 py-0.5 bg-bg-secondary dark:bg-zinc-800 text-text-secondary dark:text-gray-400 text-xs rounded-full">
                {pendingCount} {t('toolbar.pending')}
              </span>
            )}
            {processingCount > 0 && (
              <span className="px-2 py-0.5 bg-primary/10 dark:bg-primary/20 text-primary text-xs rounded-full animate-pulse-soft">
                {processingCount} {t('toolbar.processing')}
              </span>
            )}
            {completedCount > 0 && (
              <span className="px-2 py-0.5 bg-success/10 dark:bg-success/20 text-success text-xs rounded-full">
                {completedCount} {t('toolbar.completed')}
              </span>
            )}
          </div>
        )}
      </div>

      <div className="flex items-center gap-3">
        {files.length > 0 && !isProcessing && (
          <motion.button
            whileTap={{ scale: 0.97 }}
            onClick={clearFiles}
            className="flex items-center gap-1.5 px-3 py-1.5 text-sm text-text-secondary dark:text-gray-400 hover:text-danger hover:bg-danger/10 dark:hover:bg-danger/20 rounded-xl transition-colors"
          >
            <Trash2 className="w-4 h-4" />
            {t('toolbar.clear')}
          </motion.button>
        )}
      </div>
    </div>
  );
}
