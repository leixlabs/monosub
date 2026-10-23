import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import i18n from '@/i18n';
import { useStore, type UrlImportState } from '@/store/useStore';
import { addPathsToQueue } from '@/lib/fileImport';
import { reportFrontendError } from '@/lib/logging';
import { processQueueBatch } from '@/lib/transcription';

/**
 * URL import: parse the pasted text, start one job per URL, and follow the
 * `url_import_*` events (url-import-design §5.9).
 *
 * The parsing itself lives in Rust (`parse_url_input`): the frontend has no test
 * runner, and a single source of truth matters because `start_url_import` trusts
 * whatever it is given.
 */

/** Mirrors `site::url::UrlError` on the Rust side. */
export type UrlErrorTag = 'unsupported_site' | 'unsupported_kind';

/** One row of the parse result: `Ok` carries a URL, `Err` carries a reason. */
export type ParsedUrl =
  | { Ok: { input: string; url: string } }
  | { Err: { input: string; error: UrlErrorTag } };

/** One URL the dialog accepted. */
export interface AcceptedUrl {
  input: string;
  url: string;
}

/** One URL the dialog rejected, with the tag used to pick the message. */
export interface RejectedUrl {
  input: string;
  error: UrlErrorTag;
}

export interface ParseResult {
  accepted: AcceptedUrl[];
  rejected: RejectedUrl[];
  /** How many duplicates the backend dropped (design §3). */
  duplicates: number;
}

/** Split the raw IPC payload into accepted/rejected rows. */
export const parseInput = async (text: string): Promise<ParseResult> => {
  const rows = await invoke<ParsedUrl[]>('parse_url_input', { text });

  const accepted: AcceptedUrl[] = [];
  const rejected: RejectedUrl[] = [];
  const seen = new Set<string>();

  for (const row of rows) {
    if ('Ok' in row) {
      if (seen.has(row.Ok.url)) continue;
      seen.add(row.Ok.url);
      accepted.push(row.Ok);
    } else {
      rejected.push(row.Err);
    }
  }

  return { accepted, rejected, duplicates: 0 };
};

// --- Events ---------------------------------------------------------------

interface ProgressPayload {
  task_id: string;
  stage: UrlImportState['status'];
  progress?: number;
}

interface CompletePayload {
  task_id: string;
  outcome:
    | {
        outcome: 'subtitle';
        paths: string[];
        translated_paths: string[];
        tag: string;
        kind: 'human' | 'ai';
        source_lang: string;
        skipped: boolean;
      }
    | {
        outcome: 'queued';
        path: string;
        title: string;
        name: string;
        duration?: number;
        login_hint: boolean;
      };
}

interface ErrorPayload {
  task_id: string;
  status: string;
}

/** `URL_IMPORT_NEEDS_COOKIES` (url-import-design §4.5). */
export const NEEDS_COOKIES_STATUS = 'URL_IMPORT_NEEDS_COOKIES';

/**
 * Turn a backend status into the row's message.
 *
 * The backend may send a bare sentinel, an i18n key, or `key:detail` where the
 * detail is engine prose worth showing. A key that no locale defines falls back
 * to the raw string, so nothing is ever swallowed.
 */
export const describeStatus = (status: string): string => {
  if (status === NEEDS_COOKIES_STATUS) {
    return i18n.t('urlImport.errors.needsCookies');
  }

  const [key, ...rest] = status.split(':');
  const detail = rest.join(':').trim();
  if (key.startsWith('urlImport.') || key.startsWith('errors.')) {
    const translated = i18n.t(key, { defaultValue: '' });
    if (translated) {
      return detail ? `${translated}: ${detail}` : translated;
    }
  }

  return status;
};

/** Whether a status means "configure cookies and retry". */
export const isNeedsCookiesStatus = (status: string): boolean => status === NEEDS_COOKIES_STATUS;

/** Whether a status is the `MODEL_NOT_INSTALLED[:<id>]` sentinel. */
export const isModelNotInstalledStatus = (status: string): boolean =>
  status.startsWith('MODEL_NOT_INSTALLED');

let listenersAttached = false;

/**
 * Attach the `url_import_*` listeners once per process.
 *
 * `App.tsx` owns the `transcribe_*` listeners; imports have their own namespace
 * so the two never interact (design §5.9). A `Queued` outcome hands the audio to
 * the existing `fileImport` path, which is what makes an imported file
 * indistinguishable from a dropped one afterwards.
 */
export const ensureUrlImportListeners = (): (() => void) => {
  if (listenersAttached) {
    // Idempotent: React StrictMode mounts effects twice in development.
    return () => {};
  }
  listenersAttached = true;

  const unlistenProgress = listen<ProgressPayload>('url_import_progress', (event) => {
    const { task_id, stage, progress } = event.payload;
    useStore.getState().updateUrlImport(task_id, {
      status: stage,
      progress: typeof progress === 'number' ? Math.round(progress) : undefined,
    });
  });

  const unlistenComplete = listen<CompletePayload>('url_import_complete', (event) => {
    const { task_id, outcome } = event.payload;

    if (outcome.outcome === 'subtitle') {
      useStore.getState().updateUrlImport(task_id, {
        status: 'done',
        progress: 100,
        languageTag: outcome.tag,
        languageKind: outcome.kind,
        paths: outcome.paths,
        translatedPaths: outcome.translated_paths,
        skipped: outcome.skipped,
      });
      return;
    }

    // `Queued`: the import row disappears and the audio becomes a normal pending
    // file, so "start all", the batch counters and FileItem all keep working.
    void queueImportedAudio(task_id, outcome);
  });

  const unlistenError = listen<ErrorPayload>('url_import_error', (event) => {
    const { task_id, status } = event.payload;
    reportFrontendError(status, { context: `url_import_error ${task_id}` });
    useStore.getState().updateUrlImport(task_id, {
      status: 'error',
      progress: undefined,
      needsCookies: isNeedsCookiesStatus(status),
      error: describeStatus(status),
    });
  });

  return () => {
    listenersAttached = false;
    void unlistenProgress.then((off) => off());
    void unlistenComplete.then((off) => off());
    void unlistenError.then((off) => off());
  };
};

/**
 * Turn a `Queued` outcome into an ordinary queue entry.
 *
 * `addPathsToQueue` already runs `get_video_metadata` and `generate_thumbnail`,
 * so `size` and the preview come from the same place as for a local file. The two
 * extras are the display name (which keeps the video id) and `outputDir`.
 */
const queueImportedAudio = async (
  taskId: string,
  outcome: Extract<CompletePayload['outcome'], { outcome: 'queued' }>,
) => {
  const store = useStore.getState();
  const requestedName = outcome.name;
  const outputDir = resolveUrlImportOutputDir();
  // Captured before the row is removed below: it is what ties the queued file
  // back to the link, so the clipboard watcher stops offering it.
  const sourceUrl = store.urlImports[taskId]?.url;

  // The name is set explicitly rather than left to `addPathsToQueue`: the import
  // already knows the platform title, and this keeps the row showing it even if
  // the downloaded extension is not the one the metadata call reports.
  const before = new Set(store.files.map((file) => file.path));
  const result = await addPathsToQueue({
    paths: [outcome.path],
    existingFiles: store.files,
    addFiles: store.addFiles,
    source: 'UrlImport',
  });

  if (result.added > 0) {
    const added = useStore.getState().files.find((file) => !before.has(file.path));
    if (added) {
      useStore.getState().updateFile(added.id, {
        name: requestedName,
        outputDir: outputDir || undefined,
        duration: outcome.duration ?? added.duration,
        // The platform had a subtitle behind a login, so the user could have
        // skipped recognition entirely. Worth one line on the row, not an error.
        loginHint: outcome.login_hint,
        sourceUrl,
      });
    }
  } else {
    console.warn('[urlImport] the downloaded audio was already in the queue', outcome.path);
  }

  store.removeUrlImport(taskId);

  // Auto-start: purely frontend, gated on the checkbox and on nothing already
  // running (design §5.3). The backend request has no `auto_start` field.
  const current = useStore.getState();
  if (current.autoStartImports && !current.isBatchProcessing && !current.isProcessing) {
    current.setIsBatchProcessing(true);
    processQueueBatch();
  }
};

/**
 * `~/Downloads/MonoSub` when the setting is blank (design §5.4).
 *
 * The same directory serves the media and its subtitle: a downloaded video/audio
 * is the user's, so it is not parked in the app cache.
 *
 * The default is resolved by Rust (`default_url_import_dir`), not spelled out
 * here: the home directory is a platform fact, and Tauri already knows it. The
 * result is cached because the dialog and each request ask for it.
 */
let defaultOutputDir: string | null = null;

export const loadDefaultUrlImportDir = async (): Promise<string> => {
  if (defaultOutputDir) return defaultOutputDir;
  defaultOutputDir = await invoke<string>('default_url_import_dir');
  return defaultOutputDir;
};

/** The directory this app downloads imported media into and writes their subtitles into. */
export const resolveUrlImportOutputDir = (): string => {
  const state = useStore.getState();

  // `custom` mode already has an explicit directory; the URL default only
  // applies to `same`, where "next to the source" has no meaning.
  if (state.storageMode === 'custom' && state.customStoragePath.trim()) {
    return state.customStoragePath;
  }

  const configured = state.settings.urlImportOutputDir.trim();
  return configured || defaultOutputDir || '';
};

/**
 * Start one import for an accepted URL.
 *
 * Returns the task id so the caller can keep the row in sync (and offer retry).
 */
export const startUrlImport = async (url: string): Promise<string> => {
  const state = useStore.getState();
  const taskId = crypto.randomUUID();

  // Never send an empty directory: the backend would create files in the current
  // working directory. The default comes from Rust, so make sure it is loaded.
  if (!resolveUrlImportOutputDir()) {
    await loadDefaultUrlImportDir().catch((error) => {
      console.warn('[urlImport] could not resolve the default output directory', error);
    });
  }

  state.addUrlImport({
    taskId,
    input: url,
    url,
    status: 'pending',
  });

  try {
    await invoke('start_url_import', {
      request: {
        task_id: taskId,
        url,
        language: state.settings.outputLanguage,
        formats: state.settings.outputFormat,
        output_dir: resolveUrlImportOutputDir(),
        skip_existing: state.settings.skipExistingSubtitle,
        cookies_browser: state.settings.bilibiliCookiesBrowser || null,
        // The URL-import dialog has no per-run switch (there is no queue row to
        // toggle before the job starts), so it takes the app-wide default. The
        // target and model are configure-once, as everywhere else.
        translate_enabled: state.settings.translateEnabled,
        target_language: state.settings.translateTargetLanguage,
        translation_model: state.settings.translationModelId,
        prefer_local_recognition: state.settings.preferLocalRecognition,
      },
    });
  } catch (error) {
    // A rejected request never produces a `url_import_error` event, so this is
    // the only place it can be recorded.
    reportFrontendError(error, { context: `start_url_import ${taskId}` });
    state.updateUrlImport(taskId, {
      status: 'error',
      error: String(error),
    });
  }

  return taskId;
};

/** Start one import per accepted URL. */
export const startUrlImports = async (urls: AcceptedUrl[]): Promise<string[]> => {
  const ids: string[] = [];
  for (const { url } of urls) {
    ids.push(await startUrlImport(url));
  }
  return ids;
};

/** Cancel an in-flight import (only valid before it queues audio). */
export const cancelUrlImport = async (taskId: string): Promise<void> => {
  try {
    await invoke('cancel_url_import', { taskId });
  } catch (error) {
    console.warn(`[cancel_url_import] Failed for ${taskId}:`, error);
  } finally {
    useStore.getState().removeUrlImport(taskId);
  }
};

/** Retry a failed import with its own URL. */
export const retryUrlImport = async (taskId: string): Promise<string | null> => {
  const existing = useStore.getState().urlImports[taskId];
  if (!existing) return null;
  useStore.getState().removeUrlImport(taskId);
  return startUrlImport(existing.url);
};

/** Reveal a written subtitle in the system file manager. */
export const revealImportedFile = async (path: string): Promise<void> => {
  try {
    await invoke('open_in_finder', { path });
  } catch (error) {
    console.error('[open_in_finder] Failed:', error);
  }
};
