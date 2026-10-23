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

export interface ModelLicense {
  name: string;
  url: string | null;
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
  /** Deprecated single-licence string, kept for back-compat. */
  license: string;
  /** Structured licences from `models.json` `licenses[]`. */
  licenses: ModelLicense[];
  attribution: string | null;
  /** Declared roles; empty means the v1 fallback `['asr']`. */
  roles: string[];
  /** Inference engine: `audiocpp` (recognition) or `llamacpp` (translation). */
  engine: string;
  /** Prompt-template kind for a `llamacpp` translate model; `null` otherwise. */
  prompt_template: string | null;
  /** Effective timestamps grade: `none` | `turns` | `segment` | `word`. */
  timestamps: string;
  /** Effective diarization grade: `none` | `turns` | `turns_with_text`. */
  diarization: string;
  produces_text: boolean;
  source: string | null;
  tier: string | null;
  /**
   * How far the app stands behind this model.
   *
   * - `supported`   — run end to end here and known to work; ranks highest.
   * - `unverified`  — offered but never exercised here; still selectable, the
   *                   solver just ranks it below a verified one.
   * - `unsupported` — deliberately not offered; listed only so the survey reads
   *                   as one document. The solver never plans it and the
   *                   backend refuses to download it.
   *
   * Independent of `installed`: a model can be downloaded and still never
   * verified.
   */
  status: ModelSupportStatus;
  /**
   * Run modes the model's own spec declares (`offline` / `streaming`).
   *
   * The declaration only: `sense_asr` declares streaming, but this app's
   * recognition pipeline is `--request-sequence`, which `audiocpp_cli` rejects
   * outside offline mode. Use `selectable_modes` for what the UI may offer.
   */
  modes: ModelRunMode[];
  /**
   * Modes the user may actually choose here, in menu order. `['offline']` means
   * "there is no choice to make", not "a feature is missing".
   */
  selectable_modes: ModelRunMode[];
  /** The mode a run uses when nothing is stored (streaming when available). */
  default_mode: ModelRunMode;
  /**
   * The mode a run would use now: the stored choice when it is one this model
   * offers, else `default_mode`. Resolved in Rust so the panel and the pipeline
   * cannot disagree.
   */
  effective_mode: ModelRunMode;
  installed: boolean;
  size_on_disk: number;
}

/** How far the app stands behind a catalogue entry (mirrors `SupportStatus`). */
export type ModelSupportStatus = 'supported' | 'unverified' | 'unsupported';

/**
 * `audiocpp_cli --mode` (mirrors `RunMode`).
 *
 * - `offline`   — one call over the whole input; the CLI default.
 * - `streaming` — the model's own streaming policy: audio fed in chunks, state
 *                 carried across them. Lower peak memory, and speaker ids stay
 *                 consistent because the session is never restarted.
 */
export type ModelRunMode = 'offline' | 'streaming';

const asRunMode = (value: unknown): ModelRunMode =>
  value === 'streaming' ? 'streaming' : 'offline';

const asRunModeList = (value: unknown): ModelRunMode[] =>
  Array.isArray(value) && value.length > 0 ? value.map(asRunMode) : ['offline'];

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

/**
 * The catalogue id embedded in a `MODEL_NOT_INSTALLED:<id>` rejection, or `null`
 * when the sentinel carries no id (the solver had no suggestion at all).
 *
 * The id is what tells a missing recognition model apart from a missing
 * translation model on the error path (translation-design §6).
 */
export const modelNotInstalledId = (error: unknown): string | null => {
  const text = String(error);
  const marker = `${MODEL_NOT_INSTALLED_ERROR}:`;
  const index = text.indexOf(marker);
  if (index === -1) return null;
  // The id follows the marker; take the first token, ignoring quotes and any
  // wrapping around it (the sentinel is embedded in a longer message string).
  const token = text
    .slice(index + marker.length)
    .split(/[\s"'`]/)
    .find((part) => part.length > 0);
  return token ?? null;
};

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

/**
 * Read the catalogue's `status` field.
 *
 * An unrecognised or missing value means `unverified`: the model stays offered
 * (an older backend sends no field at all, and reading that as `unsupported`
 * would empty the whole list) without claiming it was ever run.
 */
const asSupportStatus = (value: unknown): ModelSupportStatus =>
  value === 'supported' || value === 'unsupported' ? value : 'unverified';

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
    licenses: Array.isArray(manifest.licenses)
      ? manifest.licenses.map((raw) => {
          const entry = isRecord(raw) ? raw : {};
          return {
            name: asString(entry.name),
            url: typeof entry.url === 'string' ? entry.url : null,
          };
        })
      : [],
    attribution: typeof manifest.attribution === 'string' ? manifest.attribution : null,
    roles: asStringArray(manifest.roles),
    // Absent `engine` means audio.cpp, matching the manifest default, so every
    // pre-translation entry reads as a recognition model here too.
    engine: asString(manifest.engine, 'audiocpp'),
    prompt_template: typeof manifest.prompt_template === 'string' ? manifest.prompt_template : null,
    timestamps: asString(manifest.timestamps, 'none'),
    diarization: asString(manifest.diarization, 'none'),
    produces_text: manifest.produces_text !== false,
    source: typeof manifest.source === 'string' ? manifest.source : null,
    tier: typeof manifest.tier === 'string' ? manifest.tier : null,
    // An unknown or absent value is read as `unverified`: the catalogue may be
    // older than this field, and that default keeps every entry offered without
    // claiming anyone proved it works.
    status: asSupportStatus(status.status),
    modes: asRunModeList(status.modes),
    selectable_modes: asRunModeList(status.selectable_modes),
    default_mode: asRunMode(status.default_mode),
    // An older backend that predates this field sends nothing, and `offline` is
    // what it would actually run, so the fallback states the truth rather than
    // guessing at streaming.
    effective_mode: asRunMode(status.effective_mode),
    installed: status.installed === true,
    size_on_disk: asNumber(status.size_on_disk),
  };
};

/**
 * True when some INSTALLED model can produce speaker information, i.e. one
 * whose `capabilities.diarization` is not `none`. Drives the BottomBar
 * multi-speaker toggle.
 *
 * `status` is part of the test: a retired model can still sit on disk from an
 * older install, and enabling the toggle on the strength of one would only fail
 * later in the run.
 */
export const hasInstalledDiarizationModel = (models: ModelStatus[]): boolean =>
  models.some(
    (model) => model.installed && model.status !== 'unsupported' && model.diarization !== 'none',
  );

/** True when some installed model can fill `role` (design §8.2). */
export const hasInstalledRole = (models: ModelStatus[], role: string): boolean =>
  models.some(
    (model) => model.installed && model.status !== 'unsupported' && model.roles.includes(role),
  );

/** The role a local translation model declares (translation-design §6). */
export const TRANSLATE_ROLE = 'translate';

/**
 * True when `model` is a translation model.
 *
 * The role is what decides, not the engine: a `llamacpp` entry without the role
 * is not something the translation stage may be pointed at (translation-design
 * §9). Keeping recognition and translation lists apart is the whole reason the
 * role exists on the wire.
 */
export const isTranslationModel = (model: ModelStatus): boolean =>
  model.roles.includes(TRANSLATE_ROLE);

/** Models the recognition plan may draw on — translation models are excluded. */
export const recognitionModels = (models: ModelStatus[]): ModelStatus[] =>
  models.filter((model) => !isTranslationModel(model));

/** Only the translation models, for the Settings translation group. */
export const translationModels = (models: ModelStatus[]): ModelStatus[] =>
  models.filter(isTranslationModel);

export const listModels = async (): Promise<ModelStatus[]> => {
  const raw = await invoke<unknown>('list_models');
  return Array.isArray(raw) ? raw.map(normalizeModelStatus) : [];
};

/**
 * Persist one model's run mode. `''` clears the preference, which restores the
 * model's default (streaming wherever the app can drive it).
 *
 * The backend re-validates both the id and the mode against the catalogue, so a
 * rejected promise here means the model does not offer that mode — not that the
 * write failed.
 */
export const setModelMode = (modelId: string, mode: ModelRunMode | ''): Promise<void> =>
  invoke<void>('set_model_mode', { modelId, mode });

export const downloadModel = (id: string): Promise<void> =>
  invoke<void>('download_model', { id });

export const cancelModelDownload = (id: string): Promise<void> =>
  invoke<void>('cancel_model_download', { id });

export const removeModel = (id: string): Promise<void> =>
  invoke<void>('remove_model', { id });

export const openModelsDir = (): Promise<void> =>
  invoke<void>('open_models_dir');

/** Where models are stored, and whether that location is the user's own. */
export interface ModelsDirInfo {
  path: string;
  isCustom: boolean;
  defaultPath: string;
  /** `env` | `settings` | null — which override is in effect. */
  source: string | null;
}

/**
 * Read the effective models directory.
 *
 * Models are large, so the user can point the app at a directory that already
 * contains them instead of downloading everything again.
 */
export const getModelsDir = (): Promise<ModelsDirInfo> =>
  invoke<ModelsDirInfo>('get_models_dir');

/**
 * Point model storage at `path`, or reset to the default when `path` is null.
 *
 * Rust validates that the directory exists and is writable; an invalid path
 * rejects with a user-showable message. Existing models are not moved.
 */
export const setModelsDir = (path: string | null): Promise<ModelsDirInfo> =>
  invoke<ModelsDirInfo>('set_models_dir', { path });

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
