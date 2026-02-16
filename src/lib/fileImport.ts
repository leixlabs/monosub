import { invoke } from '@tauri-apps/api/core';
import type { VideoFile } from '@/store/useStore';

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

const createVideoFile = async (path: string, source: string): Promise<VideoFile | null> => {
  try {
    const metadata = await invoke<{ size: number; duration: number }>('get_video_metadata', { path });
    const thumbnail = await invoke<{ thumbnail_base64?: string }>('generate_thumbnail', { path });

    return {
      id: crypto.randomUUID(),
      name: getFileName(path),
      path,
      size: metadata.size,
      duration: metadata.duration,
      thumbnail: thumbnail.thumbnail_base64,
      status: 'pending',
      progress: 0,
    };
  } catch (error) {
    const errorMessage = parseErrorMessage(error);
    console.error(`[${source}] Failed to process file:`, path, error);
    alert(`Failed to add file: ${getFileName(path)}\n\nError: ${errorMessage}\n\nPlease make sure ffmpeg and ffprobe are installed on your system.`);
    return null;
  }
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
  const supportedPaths = normalizedPaths.filter((path) => isSupportedMediaPath(path));
  const skippedUnsupported = normalizedPaths.length - supportedPaths.length;

  const processedFiles = await Promise.all(
    supportedPaths.map((path) => createVideoFile(path, source))
  );
  const validFiles = processedFiles.filter((file): file is VideoFile => file !== null);

  const uniqueFiles: VideoFile[] = [];
  const existingPaths = new Set(existingFiles.map((file) => file.path));

  for (const file of validFiles) {
    if (existingPaths.has(file.path)) {
      continue;
    }
    existingPaths.add(file.path);
    uniqueFiles.push(file);
  }

  if (uniqueFiles.length > 0) {
    addFiles(uniqueFiles);
  }

  return {
    added: uniqueFiles.length,
    skippedDuplicates: validFiles.length - uniqueFiles.length,
    skippedUnsupported,
  };
};
