import { Plus, Trash2, Film, StopCircle } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { useStore } from '@/store/useStore';
import { open } from '@tauri-apps/plugin-dialog';
import { invoke } from '@tauri-apps/api/core';
import { addPathsToQueue, MEDIA_EXTENSIONS } from '@/lib/fileImport';
import { processQueueBatch } from '@/lib/transcription';
import { GlassButton } from '@/components/ui/glass-button';
import { GlassBadge } from '@/components/ui/glass-badge';

export function Toolbar() {
  const { t } = useTranslation();
  const files = useStore((state) => state.files);
  const addFiles = useStore((state) => state.addFiles);
  const clearFiles = useStore((state) => state.clearFiles);
  const isProcessing = useStore((state) => state.isProcessing);
  const isBatchProcessing = useStore((state) => state.isBatchProcessing);
  const setIsBatchProcessing = useStore((state) => state.setIsBatchProcessing);
  const setIsProcessing = useStore((state) => state.setIsProcessing);
  const setCurrentTaskId = useStore((state) => state.setCurrentTaskId);
  const updateFile = useStore((state) => state.updateFile);

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

  const handleStartAll = async () => {
    setIsBatchProcessing(true);
    processQueueBatch();
  };

  const handleStopAll = async () => {
    setIsBatchProcessing(false);

    const processingFiles = files.filter((file) => file.status === 'processing');
    if (processingFiles.length === 0) {
      setIsProcessing(false);
      setCurrentTaskId(null);
      return;
    }

    await Promise.all(
      processingFiles.map(async (file) => {
        try {
          await invoke('cancel_transcription', { taskId: file.id });
          updateFile(file.id, {
            status: 'pending',
            progress: 0,
            error: undefined,
            stepProgress: undefined,
            totalTime: undefined,
          });
        } catch (error) {
          console.error(`[cancel_transcription] Failed for ${file.id}:`, error);
        }
      })
    );

    const stillProcessing = useStore.getState().files.some((file) => file.status === 'processing');
    if (!stillProcessing) {
      setIsProcessing(false);
      setCurrentTaskId(null);
    }
  };

  const handleBatchAction = async () => {
    if (isBatchProcessing) {
      await handleStopAll();
      return;
    }
    await handleStartAll();
  };

  return (
    <div className="bg-bg-secondary dark:bg-zinc-950 h-[64px] px-6 flex-shrink-0 sticky top-[52px] z-30 flex items-center justify-between transition-colors">
      <div className="flex items-center gap-4">
        <GlassButton
          variant="primary"
          size="sm"
          onClick={() => void handleAddFiles()}
          className="shadow-glass-md"
        >
          <Plus className="w-4 h-4" />
          {t('toolbar.addFiles')}
        </GlassButton>

        {files.length > 0 && (
          <div className="flex items-center gap-3 pl-4 border-l border-divider dark:border-zinc-700">
            <div className="flex items-center gap-1.5 text-xs text-text-secondary dark:text-gray-400">
              <Film className="w-3.5 h-3.5" />
              <span>{t('toolbar.filesCount', { count: files.length })}</span>
            </div>
            {pendingCount > 0 && (
              <GlassBadge variant="default" size="sm">
                {pendingCount} {t('toolbar.pending')}
              </GlassBadge>
            )}
            {processingCount > 0 && (
              <GlassBadge variant="primary" size="sm" className="animate-pulse-soft">
                {processingCount} {t('toolbar.processing')}
              </GlassBadge>
            )}
            {completedCount > 0 && (
              <GlassBadge variant="success" size="sm">
                {completedCount} {t('toolbar.completed')}
              </GlassBadge>
            )}
          </div>
        )}
      </div>

      <div className="flex items-center gap-3">
        {files.length > 0 && (pendingCount > 0 || isBatchProcessing) && (
          <GlassButton
            variant={isBatchProcessing ? 'destructive' : 'default'}
            size="sm"
            onClick={() => void handleBatchAction()}
          >
            {isBatchProcessing ? (
              <StopCircle className="w-4 h-4" />
            ) : (
              <Film className="w-4 h-4" />
            )}
            {isBatchProcessing ? t('bottomBar.stop') : t('toolbar.startAll')}
          </GlassButton>
        )}

        {files.length > 0 && (
          <GlassButton
            variant="ghost"
            size="sm"
            onClick={clearFiles}
            disabled={isProcessing}
            className="text-text-secondary dark:text-gray-400 hover:text-danger"
          >
            <Trash2 className="w-4 h-4" />
            {t('toolbar.clear')}
          </GlassButton>
        )}
      </div>
    </div>
  );
}
