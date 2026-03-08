import { Trash2, Play, Square, CheckCircle2, AlertCircle, FolderOpen, Loader2, Clock } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { VideoFile } from '@/store/useStore';
import { useStore } from '@/store/useStore';
import { invoke } from '@tauri-apps/api/core';
import { startTranscriptionTask } from '@/lib/transcription';
import { useEffect, useState, useRef } from 'react';
import { GlassCard } from '@/components/ui/glass-card';
import { GlassButton } from '@/components/ui/glass-button';
import { GlassBadge } from '@/components/ui/glass-badge';

interface FileItemProps {
  file: VideoFile;
}

export function FileItem({ file }: FileItemProps) {
  const { t } = useTranslation();
  const removeFile = useStore((state) => state.removeFile);
  const isProcessing = useStore((state) => state.isProcessing);
  const settings = useStore((state) => state.settings);

  // Live elapsed time counter
  const [liveElapsed, setLiveElapsed] = useState(0);
  const lastElapsedRef = useRef(0);
  const lastUpdateTimeRef = useRef(Date.now());

  useEffect(() => {
    if (file.status !== 'processing' || !file.stepProgress) {
      return;
    }

    // Update base time when we get new elapsed from backend
    lastElapsedRef.current = file.stepProgress.totalElapsed;
    lastUpdateTimeRef.current = Date.now();
    setLiveElapsed(file.stepProgress.totalElapsed);

    // Start interval to update live counter
    const interval = setInterval(() => {
      const timeSinceUpdate = (Date.now() - lastUpdateTimeRef.current) / 1000;
      setLiveElapsed(lastElapsedRef.current + timeSinceUpdate);
    }, 100);

    return () => clearInterval(interval);
  }, [file.status, file.stepProgress?.totalElapsed]);

  const formatElapsedTime = (seconds: number) => {
    const mins = Math.floor(seconds / 60);
    const secs = Math.floor(seconds % 60);
    if (mins > 0) {
      return `${mins}m ${secs}s`;
    }
    return `${secs}s`;
  };

  const handleStart = async () => {
    await startTranscriptionTask(file.id);
  };

  const handleDelete = () => {
    removeFile(file.id);
  };

  const handleCancel = async () => {
    if (file.status !== 'processing') return;

    try {
      await invoke('cancel_transcription', { taskId: file.id });
      useStore.getState().updateFile(file.id, { status: 'pending', progress: 0 });
    } catch (error) {
      console.error('[cancel_transcription] Failed:', error);
    }
  };

  const handleOpenInFinder = async () => {
    try {
      await invoke('open_in_finder', { path: file.path });
    } catch (error) {
      console.error('[open_in_finder] Failed:', error);
    }
  };

  const formatFileSize = (bytes: number) => {
    if (bytes <= 0) return '--';
    if (bytes < 1024) return bytes + ' B';
    if (bytes < 1024 * 1024) return (bytes / 1024).toFixed(1) + ' KB';
    if (bytes < 1024 * 1024 * 1024) return (bytes / (1024 * 1024)).toFixed(1) + ' MB';
    return (bytes / (1024 * 1024 * 1024)).toFixed(1) + ' GB';
  };

  const formatDuration = (seconds: number) => {
    if (seconds <= 0) return '--:--';
    const hours = Math.floor(seconds / 3600);
    const minutes = Math.floor((seconds % 3600) / 60);
    const secs = Math.floor(seconds % 60);

    if (hours > 0) {
      return `${hours}:${minutes.toString().padStart(2, '0')}:${secs.toString().padStart(2, '0')}`;
    }
    return `${minutes}:${secs.toString().padStart(2, '0')}`;
  };

  const getStatusBadge = () => {
    switch (file.status) {
      case 'pending':
        return (
          <GlassBadge variant="default" size="sm">
            {t('fileItem.pending')}
          </GlassBadge>
        );
      case 'processing':
        return (
          <div className="flex items-center gap-3">
            {file.stepProgress ? (
              <>
                {/* Step indicator */}
                <div className="flex items-center gap-2">
                  <Loader2 className="w-3.5 h-3.5 text-primary animate-spin" />
                  <span className="text-xs font-medium text-text-secondary dark:text-gray-400">
                    {file.stepProgress.currentStep}/{file.stepProgress.totalSteps}
                  </span>
                </div>
                {/* Step name */}
                <GlassBadge variant="primary" size="sm">
                  {t(`steps.${file.stepProgress.stepName.toLowerCase().replace(/\s+/g, '_')}`, file.stepProgress.stepName)}
                </GlassBadge>
                {/* Elapsed time */}
                <div className="flex items-center gap-1 text-xs text-text-tertiary dark:text-gray-500">
                  <Clock className="w-3 h-3" />
                  <span>{formatElapsedTime(liveElapsed)}</span>
                </div>
              </>
            ) : (
              <div className="flex items-center gap-2">
                <Loader2 className="w-3.5 h-3.5 text-primary animate-spin" />
                <GlassBadge variant="primary" size="sm">{t('fileItem.starting')}</GlassBadge>
              </div>
            )}
          </div>
        );
      case 'completed':
        return (
          <div className="flex items-center gap-2">
            <GlassBadge variant="success" size="sm">
              <CheckCircle2 className="w-3.5 h-3.5 mr-1" />
              {t('fileItem.completed')}
            </GlassBadge>
            {file.totalTime !== undefined && (
              <span className="text-xs text-text-tertiary dark:text-gray-500">
                {formatElapsedTime(file.totalTime)}
              </span>
            )}
          </div>
        );
      case 'skipped':
        return (
          <div className="flex items-center gap-2">
            <GlassBadge variant="default" size="sm">
              <CheckCircle2 className="w-3.5 h-3.5 mr-1" />
              {t('fileItem.skipped')}
            </GlassBadge>
          </div>
        );
      case 'error':
        return (
          <GlassBadge
            variant="destructive"
            size="sm"
            title={file.error || t('fileItem.error')}
            className="cursor-default"
          >
            <AlertCircle className="w-3.5 h-3.5 mr-1" />
            {t('fileItem.error')}
          </GlassBadge>
        );
      default:
        return null;
    }
  };

  return (
    <GlassCard
      glowEffect={file.status === 'processing'}
      className={`animate-fade-in ${file.status === 'processing' ? '' : 'hover:-translate-y-1 transition-transform duration-200'
        }`}
    >
      <div className="flex gap-4 p-4 w-full h-full">
        {/* Thumbnail */}
        <div className="w-[140px] h-[80px] rounded-lg bg-bg-secondary dark:bg-zinc-800 flex-shrink-0 overflow-hidden relative group">
          {file.thumbnail ? (
            <img
              src={file.thumbnail}
              alt={file.name}
              className="w-full h-full object-cover"
            />
          ) : (
            <div className="w-full h-full flex items-center justify-center text-text-tertiary dark:text-gray-600">
              <Play className="w-8 h-8" />
            </div>
          )}
          {/* Duration overlay */}
          <div className="absolute bottom-1.5 right-1.5 px-1.5 py-0.5 bg-black/70 dark:bg-black/80 rounded text-[10px] text-white font-medium">
            {formatDuration(file.duration)}
          </div>
        </div>

        {/* Details */}
        <div className="flex-1 flex flex-col justify-between min-w-0 py-0.5">
          <div>
            <h3 className="text-sm font-medium text-text-primary dark:text-white truncate mb-1">
              {file.name}
            </h3>
            <div className="flex items-center gap-3 text-xs text-text-tertiary dark:text-gray-500">
              <span>{formatFileSize(file.size)}</span>
              <span className="w-1 h-1 rounded-full bg-text-tertiary dark:bg-gray-600" />
              <span>{settings.outputFormat.toUpperCase()}</span>
            </div>
          </div>
          <div className="mt-2">
            {getStatusBadge()}
          </div>
        </div>

        {/* Actions */}
        <div className="flex flex-col justify-center gap-2 w-[110px]">
          {file.status === 'pending' && (
            <GlassButton
              variant="primary"
              size="sm"
              onClick={() => void handleStart()}
              disabled={isProcessing}
              className="w-full"
            >
              <Play className="w-4 h-4" />
              {t('fileItem.start')}
            </GlassButton>
          )}
          {file.status === 'processing' && (
            <GlassButton
              variant="destructive"
              size="sm"
              onClick={() => void handleCancel()}
              className="w-full"
            >
              <Square className="w-4 h-4" />
              {t('fileItem.cancel')}
            </GlassButton>
          )}
          {(file.status === 'completed' || file.status === 'skipped') && (
            <GlassButton
              variant="default"
              size="sm"
              onClick={() => void handleOpenInFinder()}
              className="w-full text-success border-success/30 hover:bg-success/20"
            >
              <FolderOpen className="w-4 h-4" />
              {t('fileItem.open')}
            </GlassButton>
          )}
          <GlassButton
            variant="ghost"
            size="sm"
            onClick={handleDelete}
            className="w-full text-text-tertiary dark:text-gray-500 hover:text-danger"
          >
            <Trash2 className="w-4 h-4" />
            {t('fileItem.delete')}
          </GlassButton>
        </div>
      </div>
    </GlassCard>
  );
}
