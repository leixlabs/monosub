import { useTranslation } from 'react-i18next';
import { useStore } from '@/store/useStore';
import { FileItem } from './FileItem';
import { Film, Plus, Subtitles } from 'lucide-react';
import { open } from '@tauri-apps/plugin-dialog';
import { invoke } from '@tauri-apps/api/core';

export function FileList() {
  const { t } = useTranslation();
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
          name: 'Media',
          extensions: ['mp4', 'mov', 'avi', 'mkv', 'webm', 'flv', 'm4v', 'mp3', 'wav', 'flac', 'aac', 'm4a', 'ogg', 'wma']
        }]
      });

      if (!selected) return;

      const paths = Array.isArray(selected) ? selected : [selected];

      const newFiles = await Promise.all(
        paths.map(async (path) => {
          try {
            console.log('[FileList] Processing file:', path);
            const metadata: any = await invoke('get_video_metadata', { path });
            console.log('[FileList] Metadata:', metadata);
            const thumbnail: any = await invoke('generate_thumbnail', { path });
            console.log('[FileList] Thumbnail generated');

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
            console.error('[FileList] Failed to process file:', path, error);
            alert(`Failed to add file: ${path.split('/').pop()}\n\nError: ${error}\n\nPlease make sure ffmpeg and ffprobe are installed on your system.`);
            return null;
          }
        })
      );

      const validFiles = newFiles.filter(f => f !== null);
      // Filter out files that already exist (by path)
      const existingPaths = new Set(files.map(f => f.path));
      const uniqueFiles = validFiles.filter(f => !existingPaths.has(f.path));

      if (uniqueFiles.length > 0) {
        addFiles(uniqueFiles);
      }

      // Notify user if some files were skipped
      const skippedCount = validFiles.length - uniqueFiles.length;
      if (skippedCount > 0) {
        console.warn(`Skipped ${skippedCount} duplicate file(s)`);
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
            <div className="absolute inset-0 bg-gradient-to-br from-primary/10 to-accent/10 rounded-3xl transform rotate-6" />
            <div className="absolute inset-0 bg-gradient-to-br from-primary/20 to-accent/20 rounded-3xl transform -rotate-3" />
            <div className="relative bg-white rounded-3xl shadow-apple-lg w-full h-full flex items-center justify-center">
              <div className="relative">
                <Film className="w-12 h-12 text-primary" />
                <Subtitles className="w-6 h-6 text-accent absolute -bottom-1 -right-2 bg-white rounded-full p-0.5" />
              </div>
            </div>
          </div>

          {/* Title */}
          <h2 className="text-xl font-semibold text-text-primary mb-2">
            {t('fileList.emptyTitle')}
          </h2>
          <p className="text-sm text-text-secondary mb-4 leading-relaxed">
            {t('fileList.emptyDescription')}
          </p>

          {/* Supported Formats */}
          <div className="mb-6 p-4 bg-bg-secondary/50 rounded-lg border border-divider">
            <div className="grid grid-cols-2 gap-4 text-xs">
              <div>
                <div className="flex items-center gap-1.5 mb-2">
                  <Film className="w-3.5 h-3.5 text-primary" />
                  <span className="font-medium text-text-primary">Video</span>
                </div>
                <div className="text-text-tertiary space-y-0.5">
                  <div>MP4, MOV, AVI, MKV</div>
                  <div>WEBM, FLV, M4V</div>
                </div>
              </div>
              <div>
                <div className="flex items-center gap-1.5 mb-2">
                  <Subtitles className="w-3.5 h-3.5 text-accent" />
                  <span className="font-medium text-text-primary">Audio</span>
                </div>
                <div className="text-text-tertiary space-y-0.5">
                  <div>MP3, WAV, FLAC, AAC</div>
                  <div>M4A, OGG, WMA</div>
                </div>
              </div>
            </div>
          </div>

          {/* Action button */}
          <button
            onClick={handleAddFiles}
            className="inline-flex items-center gap-2 px-6 py-3 bg-primary text-white text-sm font-medium rounded-xl hover:bg-primary-hover transition-all shadow-apple hover:shadow-apple-lg"
          >
            <Plus className="w-5 h-5" />
            {t('fileList.addVideos')}
          </button>

          {/* Features */}
          <div className="mt-8 flex items-center justify-center gap-6 text-xs text-text-tertiary">
            <span className="flex items-center gap-1.5">
              <span className="w-1.5 h-1.5 rounded-full bg-success" />
              {t('fileList.featureAsr')}
            </span>
            <span className="flex items-center gap-1.5">
              <span className="w-1.5 h-1.5 rounded-full bg-primary" />
              {t('fileList.featureSegment')}
            </span>
            <span className="flex items-center gap-1.5">
              <span className="w-1.5 h-1.5 rounded-full bg-accent" />
              {t('fileList.featureExport')}
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
