import { invoke } from '@tauri-apps/api/core';
import { useStore, type VideoFile } from '@/store/useStore';

export const MEDIA_EXTENSIONS = [
  'mp4',
  'mov',
  'avi',
  'mkv',
  'webm',
  'flv',
  'm4v',
  'mp3',
  'wav',
  'flac',
  'aac',
  'm4a',
  'ogg',
  'wma',
];

const mediaExtensionSet = new Set(MEDIA_EXTENSIONS);

const getFileName = (path: string): string => {
  const parts = path.split(/[\\/]/);
  return parts[parts.length - 1] || 'Unknown';
};

const getExtension = (path: string): string => {
  const fileName = getFileName(path);
  const dotIndex = fileName.lastIndexOf('.');
  return dotIndex >= 0 ? fileName.slice(dotIndex + 1).toLowerCase() : '';
};

const isSupportedMediaPath = (path: string): boolean => mediaExtensionSet.has(getExtension(path));

const parseErrorMessage = (error: unknown): string => {
  if (error instanceof Error) {
    return error.message;
  }
  return String(error);
};

const THUMBNAIL_PLACEHOLDER = `data:image/svg+xml;base64,${btoa(
  `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 320 180">
    <defs>
      <linearGradient id="g" x1="0" y1="0" x2="1" y2="1">
        <stop offset="0%" stop-color="#4FD1C5" stop-opacity="0.35"/>
        <stop offset="100%" stop-color="#60A5FA" stop-opacity="0.35"/>
      </linearGradient>
    </defs>
    <rect width="320" height="180" rx="16" fill="#111827"/>
    <rect x="1.5" y="1.5" width="317" height="177" rx="14.5" fill="url(#g)" stroke="#D1D5DB" stroke-opacity="0.25"/>
    <circle cx="160" cy="90" r="30" fill="#111827" fill-opacity="0.55"/>
    <polygon points="152,75 152,105 178,90" fill="#FFFFFF" fill-opacity="0.95"/>
  </svg>`
)}`;

const createQueuedFile = (path: string): VideoFile => ({
  id: crypto.randomUUID(),
  name: getFileName(path),
  path,
  size: 0,
  duration: 0,
  thumbnail: THUMBNAIL_PLACEHOLDER,
  status: 'pending',
  progress: 0,
  // Seeded from the app defaults; the user can flip either per file afterwards.
  diarization: useStore.getState().settings.defaultDiarization,
  translate: useStore.getState().settings.translateEnabled,
});

const enrichQueuedFile = (file: VideoFile, source: string): void => {
  const { id, path } = file;

  // Fetch metadata after the row is already visible, so batch imports feel immediate.
  void invoke<{ size: number; duration: number }>('get_video_metadata', { path })
    .then((metadata) => {
      useStore.getState().updateFile(id, {
        size: metadata.size,
        duration: metadata.duration,
      });
    })
    .catch((error) => {
      const errorMessage = parseErrorMessage(error);
      console.warn(`[${source}] Failed to get metadata for ${path}: ${errorMessage}`);
    });

  // Replace placeholder with real thumbnail asynchronously.
  void invoke<{ thumbnail_base64?: string }>('generate_thumbnail', { path })
    .then((thumbnail) => {
      if (thumbnail.thumbnail_base64) {
        useStore.getState().updateFile(id, { thumbnail: thumbnail.thumbnail_base64 });
      }
    })
    .catch((error) => {
      const errorMessage = parseErrorMessage(error);
      console.warn(`[${source}] Failed to generate thumbnail for ${path}: ${errorMessage}`);
    });
};

export interface AddPathsToQueueParams {
  paths: string[];
  existingFiles: VideoFile[];
  addFiles: (files: VideoFile[]) => void;
  source: string;
}

export interface AddPathsToQueueResult {
  added: number;
  skippedDuplicates: number;
  skippedUnsupported: number;
}

export const addPathsToQueue = async ({
  paths,
  existingFiles,
  addFiles,
  source,
}: AddPathsToQueueParams): Promise<AddPathsToQueueResult> => {
  const normalizedPaths = paths.filter((path): path is string => typeof path === 'string' && path.length > 0);

  // Resolve directories recursively via Rust backend
  const resolvedPaths: string[] = await invoke('resolve_paths', { paths: normalizedPaths });

  const supportedPaths = resolvedPaths.filter((path) => isSupportedMediaPath(path));
  const skippedUnsupported = resolvedPaths.length - supportedPaths.length;
  const queuedFiles = supportedPaths.map((path) => createQueuedFile(path));

  const uniqueFiles: VideoFile[] = [];
  const existingPaths = new Set(existingFiles.map((file) => file.path));

  for (const file of queuedFiles) {
    if (existingPaths.has(file.path)) {
      continue;
    }
    existingPaths.add(file.path);
    uniqueFiles.push(file);
  }

  if (uniqueFiles.length > 0) {
    addFiles(uniqueFiles);
    uniqueFiles.forEach((file) => enrichQueuedFile(file, source));
  }

  return {
    added: uniqueFiles.length,
    skippedDuplicates: queuedFiles.length - uniqueFiles.length,
    skippedUnsupported,
  };
};
