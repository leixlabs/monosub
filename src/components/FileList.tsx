import { useTranslation } from 'react-i18next';
import { useStore } from '@/store/useStore';
import { FileItem } from './FileItem';
import { UrlImportItem } from './UrlImportItem';
import { Film, Subtitles } from 'lucide-react';
import { addPathsToQueue } from '@/lib/fileImport';
import { CudaNotice } from './CudaNotice';

export function FileList() {
  const { t } = useTranslation();
  const files = useStore((state) => state.files);
  const addFiles = useStore((state) => state.addFiles);
  // URL imports live in their own slice: they are neither files nor part of
  // "start all", but they render above the queue so the user sees them start
  // (url-import-design §5.2).
  const urlImports = useStore((state) => state.urlImports);
  const importRows = Object.values(urlImports);

  const handleDragOver = (e: React.DragEvent) => {
    e.preventDefault();
    e.stopPropagation();
  };

  const handleDrop = async (e: React.DragEvent) => {
    e.preventDefault();
    e.stopPropagation();

    const droppedPaths = Array.from(e.dataTransfer.files)
      .map((file) => (file as File & { path?: string }).path)
      .filter((path): path is string => Boolean(path));

    if (droppedPaths.length === 0) return;

    const result = await addPathsToQueue({
      paths: droppedPaths,
      existingFiles: files,
      addFiles,
      source: 'FileListDrop',
    });

    if (result.skippedDuplicates > 0) {
      console.warn(`[FileListDrop] Skipped ${result.skippedDuplicates} duplicate file(s)`);
    }
    if (result.skippedUnsupported > 0) {
      console.warn(`[FileListDrop] Skipped ${result.skippedUnsupported} unsupported file(s)`);
    }
  };

  if (files.length === 0 && importRows.length === 0) {
    return (
      <div
        className="flex-1 min-h-0 flex items-center justify-center p-8"
        onDragOver={handleDragOver}
        onDrop={handleDrop}
      >
        <div className="text-center max-w-md w-full">
          {/* Drop zone: dashed border + icon tile is the universal "drop here"
              affordance; one calm focal point instead of stacked rotated cards. */}
          <div className="rounded-3xl border-2 border-dashed border-line-strong bg-surface/60 px-8 py-10 transition-colors duration-250 hover:border-primary/50 hover:bg-primary/[0.03]">
            <div className="mx-auto mb-5 grid place-items-center w-16 h-16 rounded-2xl bg-primary/10 text-primary ring-1 ring-inset ring-primary/15">
              <div className="relative">
                <Film className="w-7 h-7" strokeWidth={1.75} />
                <Subtitles className="w-4 h-4 absolute -bottom-1.5 -right-2 bg-surface rounded-full p-px text-accent" />
              </div>
            </div>

            <h2 className="text-xl font-semibold text-text-primary dark:text-zinc-50 mb-2">
              {t('fileList.emptyTitle')}
            </h2>
            <p className="text-sm text-text-secondary dark:text-zinc-400 mb-6 leading-relaxed text-balance">
              {t('fileList.emptyDescription')}
            </p>

            {/* Supported formats as chips: scannable, no nested card. */}
            <div className="space-y-2.5 text-xs">
              {[
                { label: 'Video', icon: <Film className="w-3.5 h-3.5 text-primary" />, exts: ['MP4', 'MOV', 'AVI', 'MKV', 'WEBM', 'FLV', 'M4V'] },
                { label: 'Audio', icon: <Subtitles className="w-3.5 h-3.5 text-accent" />, exts: ['MP3', 'WAV', 'FLAC', 'AAC', 'M4A', 'OGG', 'WMA'] },
              ].map((group) => (
                <div key={group.label} className="flex items-center gap-2 justify-center flex-wrap">
                  <span className="flex items-center gap-1.5 w-14 font-medium text-text-secondary dark:text-zinc-300">
                    {group.icon}
                    {group.label}
                  </span>
                  <div className="flex flex-wrap gap-1 justify-start">
                    {group.exts.map((ext) => (
                      <span
                        key={ext}
                        className="px-1.5 py-0.5 rounded bg-black/[0.05] dark:bg-white/[0.07] text-text-tertiary dark:text-zinc-400 font-medium tabular-nums"
                      >
                        {ext}
                      </span>
                    ))}
                  </div>
                </div>
              ))}
            </div>
          </div>

          <p className="mt-5 text-sm text-text-tertiary dark:text-zinc-500">
            {t('fileList.addVideos')}
          </p>

          {/* Windows only, and only when it has something to say: the CUDA build
              is the sole engine there, so a missing driver is worth flagging
              *before* the first file fails. Renders nothing elsewhere. */}
          <CudaNotice />

          {/* Features */}
          <div className="mt-6 flex items-center justify-center gap-5 text-xs text-text-tertiary dark:text-zinc-500">
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
    <div className="flex-1 min-h-0 overflow-y-auto px-6 pt-2 pb-6">
      <div className="space-y-2.5">
        {importRows.map((urlImport, index) => (
          <div
            key={urlImport.taskId}
            style={{ animationDelay: `${index * 50}ms` }}
            className="animate-in"
          >
            <UrlImportItem urlImport={urlImport} />
          </div>
        ))}
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
