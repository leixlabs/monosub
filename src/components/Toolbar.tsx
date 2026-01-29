import { Plus, Trash2, Film } from 'lucide-react';
import { useStore } from '@/store/useStore';
import { open } from '@tauri-apps/plugin-dialog';
import { invoke } from '@tauri-apps/api/core';

export function Toolbar() {
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
          name: 'Video',
          extensions: ['mp4', 'mov', 'avi', 'mkv', 'webm', 'flv', 'm4v']
        }]
      });

      if (!selected) return;

      const paths = Array.isArray(selected) ? selected : [selected];

      const newFiles = await Promise.all(
        paths.map(async (path) => {
          try {
            const metadata: any = await invoke('get_video_metadata', { path });
            const thumbnail: any = await invoke('generate_thumbnail', { path });

            return {
              id: crypto.randomUUID(),
              name: path.split('/').pop() || 'Unknown',
              path,
              size: metadata.size,
              duration: metadata.duration,
              thumbnail: thumbnail.thumbnail_base64,
              status: 'pending' as const,
              progress: 0,
            };
          } catch (error) {
            console.error('Failed to process file:', error);
            return null;
          }
        })
      );

      const validFiles = newFiles.filter(f => f !== null);
      if (validFiles.length > 0) {
        addFiles(validFiles);
      }
    } catch (error) {
      console.error('Failed to select files:', error);
    }
  };

  return (
    <div className="bg-bg-card/80 backdrop-blur-lg h-[64px] px-6 flex items-center justify-between border-b border-divider">
      <div className="flex items-center gap-4">
        <button
          onClick={handleAddFiles}
          className="flex items-center gap-2 px-4 py-2 bg-primary text-white text-sm font-medium rounded-lg hover:bg-primary-hover transition-colors shadow-apple"
        >
          <Plus className="w-4 h-4" />
          添加文件
        </button>

        {files.length > 0 && (
          <div className="flex items-center gap-3 pl-4 border-l border-divider">
            <div className="flex items-center gap-1.5 text-xs text-text-secondary">
              <Film className="w-3.5 h-3.5" />
              <span>{files.length} 个文件</span>
            </div>
            {pendingCount > 0 && (
              <span className="px-2 py-0.5 bg-bg-secondary text-text-secondary text-xs rounded-full">
                {pendingCount} 待处理
              </span>
            )}
            {processingCount > 0 && (
              <span className="px-2 py-0.5 bg-primary/10 text-primary text-xs rounded-full animate-pulse-soft">
                {processingCount} 处理中
              </span>
            )}
            {completedCount > 0 && (
              <span className="px-2 py-0.5 bg-success/10 text-success text-xs rounded-full">
                {completedCount} 已完成
              </span>
            )}
          </div>
        )}
      </div>

      <div className="flex items-center gap-3">
        {files.length > 0 && !isProcessing && (
          <button
            onClick={clearFiles}
            className="flex items-center gap-1.5 px-3 py-1.5 text-sm text-text-secondary hover:text-danger hover:bg-danger/10 rounded-lg transition-colors"
          >
            <Trash2 className="w-4 h-4" />
            清空
          </button>
        )}
      </div>
    </div>
  );
}
