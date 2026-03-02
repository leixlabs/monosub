import { Trash2, Play, Square, CheckCircle2, AlertCircle, FolderOpen, Loader2, Clock } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { motion } from 'framer-motion';
import { VideoFile } from '@/store/useStore';
import { useStore } from '@/store/useStore';
import { invoke } from '@tauri-apps/api/core';
import { startTranscriptionTask } from '@/lib/transcription';
import { useEffect, useState, useRef } from 'react';

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
    if (bytes < 1024) return bytes + ' B';
    if (bytes < 1024 * 1024) return (bytes / 1024).toFixed(1) + ' KB';
    if (bytes < 1024 * 1024 * 1024) return (bytes / (1024 * 1024)).toFixed(1) + ' MB';
    return (bytes / (1024 * 1024 * 1024)).toFixed(1) + ' GB';
  };

  const formatDuration = (seconds: number) => {
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
          <span className="inline-flex items-center px-2.5 py-1 rounded-full text-xs font-medium bg-bg-secondary dark:bg-zinc-800 text-text-secondary dark:text-gray-400">
            {t('fileItem.pending')}
          </span>
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
                <span className="text-xs font-medium text-primary dark:text-primary">
                  {t(`steps.${file.stepProgress.stepName.toLowerCase().replace(/\s+/g, '_')}`, file.stepProgress.stepName)}
                </span>
                {/* Elapsed time */}
                <div className="flex items-center gap-1 text-xs text-text-tertiary dark:text-gray-500">
                  <Clock className="w-3 h-3" />
                  <span>{formatElapsedTime(liveElapsed)}</span>
                </div>
              </>
            ) : (
              <div className="flex items-center gap-2">
                <Loader2 className="w-3.5 h-3.5 text-primary animate-spin" />
                <span className="text-xs font-medium text-primary dark:text-primary">{t('fileItem.starting')}</span>
              </div>
            )}
          </div>
        );
      case 'completed':
        return (
          <div className="flex items-center gap-2">
            <span className="inline-flex items-center gap-1.5 px-2.5 py-1 rounded-full text-xs font-medium bg-success/10 dark:bg-success/20 text-success">
              <CheckCircle2 className="w-3.5 h-3.5" />
              {t('fileItem.completed')}
            </span>
            {file.totalTime && (
              <span className="text-xs text-text-tertiary dark:text-gray-500">
                {formatElapsedTime(file.totalTime)}
              </span>
            )}
          </div>
        );
      case 'error':
        return (
          <span
            className="inline-flex items-center gap-1.5 px-2.5 py-1 rounded-full text-xs font-medium bg-danger/10 dark:bg-danger/20 text-danger cursor-default"
            title={file.error || t('fileItem.error')}
          >
            <AlertCircle className="w-3.5 h-3.5" />
            {t('fileItem.error')}
          </span>
        );
      default:
        return null;
    }
  };

  return (
    <motion.div
      whileHover={{ y: -4 }}
      transition={{ duration: 0.2 }}
      className={`
        glass-medium dark:glass-dark-strong rounded-2xl p-4 flex gap-4 animate-fade-in
        ${file.status === 'processing' ? 'shadow-float-hover' : 'shadow-float hover:shadow-float-hover'}
      `}
    >
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
      <div className="flex flex-col justify-center gap-2 w-[100px]">
        {file.status === 'pending' && (
          <motion.button
            whileTap={{ scale: 0.97 }}
            onClick={handleStart}
            disabled={isProcessing}
            className="flex items-center justify-center gap-1.5 px-4 py-2 bg-primary text-white text-sm font-medium rounded-xl hover:bg-primary-hover transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
          >
            <Play className="w-4 h-4" />
            {t('fileItem.start')}
          </motion.button>
        )}
        {file.status === 'processing' && (
          <motion.button
            whileTap={{ scale: 0.97 }}
            onClick={handleCancel}
            className="flex items-center justify-center gap-1.5 px-4 py-2 border border-red-200 dark:border-red-800 text-red-500 dark:text-red-400 text-sm font-medium rounded-xl hover:bg-red-50 dark:hover:bg-red-900/20 transition-colors"
          >
            <Square className="w-4 h-4" />
            {t('fileItem.cancel')}
          </motion.button>
        )}
        {file.status === 'completed' && (
          <motion.button
            whileTap={{ scale: 0.97 }}
            onClick={handleOpenInFinder}
            className="flex items-center justify-center gap-1.5 px-4 py-2 bg-success/10 dark:bg-success/20 text-success text-sm font-medium rounded-xl hover:bg-success/20 dark:hover:bg-success/30 transition-colors"
          >
            <FolderOpen className="w-4 h-4" />
            {t('fileItem.open')}
          </motion.button>
        )}
        <motion.button
          whileTap={{ scale: 0.97 }}
          onClick={handleDelete}
          className="flex items-center justify-center gap-1.5 px-4 py-2 text-text-tertiary dark:text-gray-500 text-sm rounded-xl hover:bg-danger/10 dark:hover:bg-danger/20 hover:text-danger transition-colors"
        >
          <Trash2 className="w-4 h-4" />
          {t('fileItem.delete')}
        </motion.button>
      </div>
    </motion.div>
  );
}
