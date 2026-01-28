import { Plus, Trash2 } from 'lucide-react';
import { useStore } from '@/store/useStore';
import { open } from '@tauri-apps/plugin-dialog';
import { invoke } from '@tauri-apps/api/core';

export function Toolbar() {
  const files = useStore((state) => state.files);
  const addFiles = useStore((state) => state.addFiles);
  const clearFiles = useStore((state) => state.clearFiles);

  const handleAddFiles = async () => {
    try {
      const selected = await open({
        multiple: true,
        filters: [{
          name: 'Video',
          extensions: ['mp4', 'mov', 'avi', 'mkv', 'webm']
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
    <div className="bg-bg-card h-[72px] px-6 flex items-center justify-between border-b border-border">
      <div className="flex items-center gap-3">
        <button
          onClick={handleAddFiles}
          className="flex items-center gap-2 px-4 py-2.5 bg-primary text-white rounded-md hover:bg-primary-hover transition-colors"
        >
          <Plus className="w-4 h-4" />
          <span className="text-sm font-medium">添加文件</span>
        </button>
      </div>

      <div className="flex items-center gap-4">
        <span className="text-sm text-text-secondary">
          共 {files.length} 个
        </span>
        {files.length > 0 && (
          <button
            onClick={clearFiles}
            className="flex items-center gap-2 px-3.5 py-2 border border-border rounded-md hover:bg-bg-secondary transition-colors"
          >
            <Trash2 className="w-4 h-4" />
            <span className="text-sm">清空</span>
          </button>
        )}
      </div>
    </div>
  );
}
