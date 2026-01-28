import { Trash2, Play, Pause, CheckCircle2, AlertCircle } from 'lucide-react';
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

  const handleStart = async () => {
    if (file.status === 'processing') return;

    const outputPath = storageMode === 'same'
      ? file.path.replace(/\.[^/.]+$/, '.srt')
      : `${customStoragePath}/${file.name.replace(/\.[^/.]+$/, '.srt')}`;

    console.log('[handleStart]', { id: file.id, path: file.path, outputPath });

    updateFile(file.id, { status: 'processing', progress: 0 });
    setIsProcessing(true);

    try {
      const result = await invoke('start_transcription', {
        request: {
          id: file.id,
          path: file.path,
          output_path: outputPath,
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
          <span className="text-xs text-text-tertiary">
            等待处理
          </span>
        );
      case 'processing':
        return (
          <div className="flex items-center gap-2">
            <div className="w-24 h-1.5 bg-bg-secondary rounded-full overflow-hidden">
              <div
                className="h-full bg-primary transition-all duration-300"
                style={{ width: `${file.progress}%` }}
              />
            </div>
            <span className="text-xs text-text-secondary">{file.progress}%</span>
          </div>
        );
      case 'completed':
        return (
          <div className="flex items-center gap-1.5 text-success">
            <CheckCircle2 className="w-4 h-4" />
            <span className="text-xs">已完成</span>
          </div>
        );
      case 'error':
        return (
          <div className="flex items-center gap-1.5 text-danger">
            <AlertCircle className="w-4 h-4" />
            <span className="text-xs">错误</span>
          </div>
        );
      default:
        return null;
    }
  };

  return (
    <div className="bg-bg-card border border-border rounded-md p-4 flex gap-4">
      {/* Thumbnail */}
      <div className="w-[120px] h-[90px] rounded-sm bg-bg-secondary flex-shrink-0 overflow-hidden">
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
      </div>

      {/* Details */}
      <div className="flex-1 flex flex-col gap-2 min-w-0">
        <h3 className="text-sm font-medium text-text-primary truncate">
          {file.name}
        </h3>
        <div className="flex items-center gap-4 text-xs text-text-secondary">
          <span>{formatFileSize(file.size)}</span>
          <span>{formatDuration(file.duration)}</span>
        </div>
        <div className="mt-auto">
          {getStatusBadge()}
        </div>
      </div>

      {/* Actions */}
      <div className="flex flex-col gap-3 w-[120px]">
        {file.status === 'pending' && (
          <button
            onClick={handleStart}
            disabled={isProcessing}
            className="flex items-center justify-center gap-2 px-4 py-2 bg-primary text-white rounded-md hover:bg-primary-hover transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
          >
            <Play className="w-4 h-4" />
            <span className="text-sm">开始</span>
          </button>
        )}
        {file.status === 'processing' && (
          <button
            className="flex items-center justify-center gap-2 px-4 py-2 border border-border rounded-md hover:bg-bg-secondary transition-colors"
          >
            <Pause className="w-4 h-4" />
            <span className="text-sm">暂停</span>
          </button>
        )}
        <button
          onClick={handleDelete}
          className="flex items-center justify-center gap-2 px-4 py-2 border border-border rounded-md hover:bg-bg-secondary transition-colors text-danger"
        >
          <Trash2 className="w-4 h-4" />
          <span className="text-sm">删除</span>
        </button>
      </div>
    </div>
  );
}
