import { Trash2, Play, Pause, CheckCircle2, AlertCircle, FolderOpen, Loader2 } from 'lucide-react';
import { VideoFile } from '@/store/useStore';
import { useStore } from '@/store/useStore';
import { invoke } from '@tauri-apps/api/core';

interface FileItemProps {
  file: VideoFile;
}

export function FileItem({ file }: FileItemProps) {
  const removeFile = useStore((state) => state.removeFile);
  const updateFile = useStore((state) => state.updateFile);
  const isProcessing = useStore((state) => state.isProcessing);
  const setIsProcessing = useStore((state) => state.setIsProcessing);
  const storageMode = useStore((state) => state.storageMode);
  const customStoragePath = useStore((state) => state.customStoragePath);
  const settings = useStore((state) => state.settings);

  const handleStart = async () => {
    if (file.status === 'processing') return;

    const extension = `.${settings.outputFormat}`;
    const outputPath = storageMode === 'same'
      ? file.path.replace(/\.[^/.]+$/, extension)
      : `${customStoragePath}/${file.name.replace(/\.[^/.]+$/, extension)}`;

    console.log('[handleStart]', { id: file.id, path: file.path, outputPath });

    updateFile(file.id, { status: 'processing', progress: 0 });
    setIsProcessing(true);

    try {
      const result = await invoke('start_transcription', {
        request: {
          id: file.id,
          path: file.path,
          output_path: outputPath,
          format: settings.outputFormat,
          language: settings.outputLanguage,
        }
      });
      console.log('[start_transcription] Success:', result);
    } catch (error) {
      console.error('[start_transcription] Failed:', error);
      updateFile(file.id, {
        status: 'error',
        error: String(error)
      });
      setIsProcessing(false);
    }
  };

  const handleDelete = () => {
    removeFile(file.id);
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
          <span className="inline-flex items-center px-2.5 py-1 rounded-full text-xs font-medium bg-bg-secondary text-text-secondary">
            等待处理
          </span>
        );
      case 'processing':
        return (
          <div className="flex items-center gap-3">
            <div className="flex items-center gap-2">
              <Loader2 className="w-3.5 h-3.5 text-primary animate-spin" />
              <span className="text-xs font-medium text-primary">{file.progress}%</span>
            </div>
            <div className="w-32 h-2 bg-bg-secondary rounded-full overflow-hidden">
              <div
                className="h-full bg-gradient-to-r from-primary to-info transition-all duration-300 ease-out"
                style={{ width: `${file.progress}%` }}
              />
            </div>
          </div>
        );
      case 'completed':
        return (
          <span className="inline-flex items-center gap-1.5 px-2.5 py-1 rounded-full text-xs font-medium bg-success/10 text-success">
            <CheckCircle2 className="w-3.5 h-3.5" />
            已完成
          </span>
        );
      case 'error':
        return (
          <span className="inline-flex items-center gap-1.5 px-2.5 py-1 rounded-full text-xs font-medium bg-danger/10 text-danger">
            <AlertCircle className="w-3.5 h-3.5" />
            错误
          </span>
        );
      default:
        return null;
    }
  };

  return (
    <div className={`
      bg-bg-card border rounded-lg p-4 flex gap-4 transition-all duration-200 animate-fade-in
      ${file.status === 'processing' ? 'border-primary/30 shadow-apple' : 'border-border hover:border-primary/20 hover:shadow-apple'}
    `}>
      {/* Thumbnail */}
      <div className="w-[140px] h-[80px] rounded-md bg-bg-secondary flex-shrink-0 overflow-hidden relative group">
        {file.thumbnail ? (
          <img
            src={file.thumbnail}
            alt={file.name}
            className="w-full h-full object-cover"
          />
        ) : (
          <div className="w-full h-full flex items-center justify-center text-text-tertiary">
            <Play className="w-8 h-8" />
          </div>
        )}
        {/* Duration overlay */}
        <div className="absolute bottom-1.5 right-1.5 px-1.5 py-0.5 bg-black/70 rounded text-[10px] text-white font-medium">
          {formatDuration(file.duration)}
        </div>
      </div>

      {/* Details */}
      <div className="flex-1 flex flex-col justify-between min-w-0 py-0.5">
        <div>
          <h3 className="text-sm font-medium text-text-primary truncate mb-1">
            {file.name}
          </h3>
          <div className="flex items-center gap-3 text-xs text-text-tertiary">
            <span>{formatFileSize(file.size)}</span>
            <span className="w-1 h-1 rounded-full bg-text-tertiary" />
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
          <button
            onClick={handleStart}
            disabled={isProcessing}
            className="flex items-center justify-center gap-1.5 px-4 py-2 bg-primary text-white text-sm font-medium rounded-lg hover:bg-primary-hover transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
          >
            <Play className="w-4 h-4" />
            开始
          </button>
        )}
        {file.status === 'processing' && (
          <button
            className="flex items-center justify-center gap-1.5 px-4 py-2 border border-border text-text-secondary text-sm font-medium rounded-lg hover:bg-bg-secondary transition-colors"
          >
            <Pause className="w-4 h-4" />
            暂停
          </button>
        )}
        {file.status === 'completed' && (
          <button
            onClick={handleOpenInFinder}
            className="flex items-center justify-center gap-1.5 px-4 py-2 bg-success/10 text-success text-sm font-medium rounded-lg hover:bg-success/20 transition-colors"
          >
            <FolderOpen className="w-4 h-4" />
            打开
          </button>
        )}
        <button
          onClick={handleDelete}
          className="flex items-center justify-center gap-1.5 px-4 py-2 text-text-tertiary text-sm rounded-lg hover:bg-danger/10 hover:text-danger transition-colors"
        >
          <Trash2 className="w-4 h-4" />
          删除
        </button>
      </div>
    </div>
  );
}
