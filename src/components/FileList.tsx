import { useStore } from '@/store/useStore';
import { FileItem } from './FileItem';
import { Film, Plus, Subtitles } from 'lucide-react';
import { open } from '@tauri-apps/plugin-dialog';
import { invoke } from '@tauri-apps/api/core';

export function FileList() {
  const files = useStore((state) => state.files);
  const addFiles = useStore((state) => state.addFiles);

  const handleDragOver = (e: React.DragEvent) => {
    e.preventDefault();
    e.stopPropagation();
  };

  const handleDrop = async (e: React.DragEvent) => {
    e.preventDefault();
    e.stopPropagation();

    // Note: Tauri handles file drops differently
    // This is a placeholder for future drag-and-drop implementation
    console.log('Drop event received');
  };

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

  if (files.length === 0) {
    return (
      <div
        className="flex-1 flex items-center justify-center p-8"
        onDragOver={handleDragOver}
        onDrop={handleDrop}
      >
        <div className="text-center max-w-md">
          {/* Illustration */}
          <div className="relative w-32 h-32 mx-auto mb-6">
            <div className="absolute inset-0 bg-gradient-to-br from-primary/10 to-info/10 rounded-3xl transform rotate-6" />
            <div className="absolute inset-0 bg-gradient-to-br from-primary/20 to-info/20 rounded-3xl transform -rotate-3" />
            <div className="relative bg-white rounded-3xl shadow-apple-lg w-full h-full flex items-center justify-center">
              <div className="relative">
                <Film className="w-12 h-12 text-primary" />
                <Subtitles className="w-6 h-6 text-info absolute -bottom-1 -right-2 bg-white rounded-full p-0.5" />
              </div>
            </div>
          </div>

          {/* Title */}
          <h2 className="text-xl font-semibold text-text-primary mb-2">
            开始生成字幕
          </h2>
          <p className="text-sm text-text-secondary mb-6 leading-relaxed">
            添加视频文件，AI 将自动识别语音并生成字幕
            <br />
            <span className="text-text-tertiary">支持 MP4、MOV、AVI、MKV 等格式</span>
          </p>

          {/* Action button */}
          <button
            onClick={handleAddFiles}
            className="inline-flex items-center gap-2 px-6 py-3 bg-primary text-white text-sm font-medium rounded-xl hover:bg-primary-hover transition-all shadow-apple hover:shadow-apple-lg"
          >
            <Plus className="w-5 h-5" />
            添加视频文件
          </button>

          {/* Features */}
          <div className="mt-8 flex items-center justify-center gap-6 text-xs text-text-tertiary">
            <span className="flex items-center gap-1.5">
              <span className="w-1.5 h-1.5 rounded-full bg-success" />
              自动语音识别
            </span>
            <span className="flex items-center gap-1.5">
              <span className="w-1.5 h-1.5 rounded-full bg-primary" />
              智能断句
            </span>
            <span className="flex items-center gap-1.5">
              <span className="w-1.5 h-1.5 rounded-full bg-info" />
              多格式导出
            </span>
          </div>
        </div>
      </div>
    );
  }

  return (
    <div className="flex-1 overflow-y-auto p-6">
      <div className="space-y-3">
        {files.map((file, index) => (
          <div
            key={file.id}
            style={{ animationDelay: `${index * 50}ms` }}
            className="animate-in"
          >
            <FileItem file={file} />
          </div>
        ))}
      </div>
    </div>
  );
}
