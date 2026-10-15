import { invoke } from '@tauri-apps/api/core';
import { SETTINGS_OPEN_CATEGORY_STORAGE_KEY } from '@/lib/license';
import { useStore } from '@/store/useStore';

/**
 * Frontend wrapper over the audio.cpp model-management command layer
 * (docs/tech-spec/refactor-audiocpp.md §5.2).
 *
 * Shapes mirror the committed manifest `src-tauri/resources/models.json`; the
 * Rust side (`commands/models.rs`) returns the same fields plus `installed` and
 * `size_on_disk`.
 */

/** One `files[]` entry of the model manifest. */
export interface ModelFileManifest {
  path: string;
  /** Primary URL plus fallback mirrors, tried in order by the downloader. */
  urls: string[];
  size: number;
  sha256: string | null;
}

export interface ModelStatus {
  id: string;
  /** audio.cpp family, e.g. `sense_asr`. */
  family: string;
  display_name: string;
  target_directory: string;
  model_file: string;
  files: ModelFileManifest[];
  languages: string[];
  license: string;
  attribution: string | null;
  installed: boolean;
  size_on_disk: number;
}

export interface ModelDownloadProgressPayload {
  id: string;
  downloaded: number;
  total: number;
}

export interface ModelDownloadCompletePayload {
  id: string;
}

export interface ModelDownloadErrorPayload {
  id: string;
  message: string;
}

export const MODEL_DOWNLOAD_PROGRESS_EVENT = 'model_download_progress';
export const MODEL_DOWNLOAD_COMPLETE_EVENT = 'model_download_complete';
export const MODEL_DOWNLOAD_ERROR_EVENT = 'model_download_error';

/** Sentinel returned by `start_transcription` when the default model is absent. */
export const MODEL_NOT_INSTALLED_ERROR = 'MODEL_NOT_INSTALLED';

/** Settings category that hosts the model list (see `Settings.tsx`). */
export const MODELS_SETTINGS_CATEGORY = 'models';

const isRecord = (value: unknown): value is Record<string, unknown> =>
  typeof value === 'object' && value !== null;

const asString = (value: unknown, fallback = ''): string =>
  typeof value === 'string' ? value : fallback;

const asNumber = (value: unknown, fallback = 0): number =>
  typeof value === 'number' && Number.isFinite(value) ? value : fallback;

const asStringArray = (value: unknown): string[] =>
  Array.isArray(value) ? value.filter((item): item is string => typeof item === 'string') : [];

const normalizeFile = (raw: unknown): ModelFileManifest => {
  const file = isRecord(raw) ? raw : {};
  const urls = asStringArray(file.urls);
  const primary = asString(file.url);
  return {
    path: asString(file.path),
    // manifests may carry either `urls: []` or a single `url`.
    urls: urls.length > 0 ? urls : (primary ? [primary] : []),
    size: asNumber(file.size),
    sha256: typeof file.sha256 === 'string' ? file.sha256 : null,
  };
};

/**
 * `list_models` returns flat manifest fields. The catalogue type in
 * `engine/catalog.rs` wraps them in a `spec` object, so unwrap it when present
 * instead of rendering an empty list.
 */
const normalizeModelStatus = (raw: unknown): ModelStatus => {
  const status = isRecord(raw) ? raw : {};
  const manifest = isRecord(status.spec) ? status.spec : status;

  return {
    id: asString(manifest.id),
    family: asString(manifest.family),
    display_name: asString(manifest.display_name),
    target_directory: asString(manifest.target_directory),
    model_file: asString(manifest.model_file),
    files: Array.isArray(manifest.files) ? manifest.files.map(normalizeFile) : [],
    languages: asStringArray(manifest.languages),
    license: asString(manifest.license),
    attribution: typeof manifest.attribution === 'string' ? manifest.attribution : null,
    installed: status.installed === true,
    size_on_disk: asNumber(status.size_on_disk),
  };
};

export const listModels = async (): Promise<ModelStatus[]> => {
  const raw = await invoke<unknown>('list_models');
  return Array.isArray(raw) ? raw.map(normalizeModelStatus) : [];
};

export const downloadModel = (id: string): Promise<void> =>
  invoke<void>('download_model', { id });

export const cancelModelDownload = (id: string): Promise<void> =>
  invoke<void>('cancel_model_download', { id });

export const removeModel = (id: string): Promise<void> =>
  invoke<void>('remove_model', { id });

export const openModelsDir = (): Promise<void> =>
  invoke<void>('open_models_dir');

/** Download size declared by the manifest (independent of what is on disk). */
export const modelManifestSize = (model: ModelStatus): number =>
  model.files.reduce((total, file) => total + asNumber(file.size), 0);

export const formatModelSize = (bytes: number): string => {
  if (!Number.isFinite(bytes) || bytes <= 0) return '—';
  const units = ['B', 'KB', 'MB', 'GB', 'TB'];
  let value = bytes;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit += 1;
  }
  const digits = value >= 100 || unit === 0 ? 0 : 1;
  return `${value.toFixed(digits)} ${units[unit]}`;
};

/** Download percentage as a whole number, safe when `total` is unknown. */
export const downloadPercent = (downloaded: number, total: number): number => {
  if (!Number.isFinite(total) || total <= 0) return 0;
  return Math.max(0, Math.min(100, Math.round((downloaded / total) * 100)));
};

/** Open Settings on the Models category (used when a model is missing). */
export const openModelsSettings = (): void => {
  try {
    localStorage.setItem(SETTINGS_OPEN_CATEGORY_STORAGE_KEY, MODELS_SETTINGS_CATEGORY);
  } catch (error) {
    console.warn('[openModelsSettings] Failed to persist target category:', error);
  }
  useStore.getState().setIsSettingsOpen(true);
};
