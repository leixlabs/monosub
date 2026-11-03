/**
 * Directory grants — remembering a folder the user allowed.
 *
 * The open panel hands the app access to a folder **for this process only**. On
 * macOS (App Sandbox) that access can be turned into a security-scoped bookmark,
 * but only in the instant right after the panel returns — so every picker calls
 * [`authorizeDir`] before doing anything else with the path, and the backend
 * replays the stored bookmarks at launch.
 *
 * Two rules this module exists to enforce:
 *
 * 1. ask **once** — a folder the user picked keeps working after a restart,
 * 2. never fail silently — when a folder turns out to be unwritable the caller
 *    gets the folder's path back ([`outputDeniedDir`]) and can ask for it again,
 *    instead of the app quietly writing somewhere else.
 */
import { invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';
import i18n from '@/i18n';
import { useStore } from '@/store/useStore';

/** Prefix the Rust side puts on a "cannot write here" status (see `paths.rs`). */
const NOT_WRITABLE = 'OUTPUT_NOT_WRITABLE:';

/** Prefix for "the models folder cannot be read" (also `paths.rs`). */
const MODELS_NOT_ACCESSIBLE = 'MODELS_DIR_NOT_ACCESSIBLE:';

/**
 * Remember access to a folder the user just picked.
 *
 * Failures are logged, not thrown: on platforms without the sandbox the backend
 * is a no-op, and a bookmark that could not be made must not stop the user from
 * saving a file *now* (it only costs them a second grant next launch).
 */
export async function authorizeDir(path: string): Promise<void> {
  try {
    await invoke('authorize_dir', { path });
  } catch (error) {
    console.warn('[grants] could not remember access to', path, error);
  }
}

/** `OUTPUT_NOT_WRITABLE:/Users/…` → `/Users/…`, or `null` for any other status. */
export const outputDeniedDir = (status: string): string | null => {
  if (!status.startsWith(NOT_WRITABLE)) return null;
  const dir = status.slice(NOT_WRITABLE.length).trim();
  return dir.length > 0 ? dir : null;
};

/** `MODELS_DIR_NOT_ACCESSIBLE:/Users/…` → `/Users/…`, or `null`. */
export const modelsDirDeniedPath = (status: string): string | null => {
  if (!status.startsWith(MODELS_NOT_ACCESSIBLE)) return null;
  const dir = status.slice(MODELS_NOT_ACCESSIBLE.length).trim();
  return dir.length > 0 ? dir : null;
};

/**
 * Ask the user for a save folder, starting at the one that was refused, and
 * remember the choice.
 *
 * `null` means the user cancelled — the caller must then show an error rather
 * than retry, because retrying would immediately hit the same denial.
 */
export async function requestOutputFolder(deniedDir: string): Promise<string | null> {
  const selected = await open({
    directory: true,
    multiple: false,
    defaultPath: deniedDir,
  });
  if (!selected || typeof selected !== 'string') return null;
  await authorizeDir(selected);
  return selected;
}

// ------------------------------------------------------------ sandbox output access
//
// 沙盒下"输出到视频旁边"仍然保留，只是**写不进去**：用户选中的是文件，那个
// 文件的父目录对应用仍然只读（见 `paths.rs::is_sandboxed`）。要把它变成可写，
// 只能让用户对那个目录授权一次，并把它存成 security-scoped bookmark。
//
// 顺序是刻意的：先解释（应用内对话框，`FolderGrantDialog`），再弹系统面板。
// 直接甩一个系统目录面板，用户看到的是"我明明选过文件了，怎么又要选目录"；
// 先说清为什么、并且只问一次，同一个动作为才有意义。

/** Set once the backend has answered; `null` means "not asked yet". */
let sandboxKnown: boolean | null = null;

/**
 * Learn whether this build is sandboxed, and publish it to the store.
 *
 * A failed probe falls back to "not sandboxed" — the DMG build is the common
 * case, and its behaviour is the one that never asks for anything.
 */
export async function bootstrapSandboxFlag(): Promise<boolean> {
  if (sandboxKnown !== null) return sandboxKnown;
  let sandboxed = false;
  try {
    sandboxed = await invoke<boolean>('app_sandboxed');
  } catch (error) {
    console.warn('[grants] could not tell whether the app is sandboxed', error);
  }
  sandboxKnown = sandboxed;
  useStore.getState().setIsSandboxed(sandboxed);
  return sandboxed;
}

/** Can the app create a file in `dir` right now? Probed, not assumed. */
async function isWritable(dir: string): Promise<boolean> {
  try {
    return await invoke<boolean>('output_dir_writable', { path: dir });
  } catch (error) {
    console.warn('[grants] could not probe', dir, error);
    return false;
  }
}

/** `/a/b/episode.mp4` → `/a/b`. `null` when the path has no directory part. */
export function parentDir(path: string): string | null {
  const trimmed = path.replace(/\/+$/, '');
  const cut = trimmed.lastIndexOf('/');
  return cut > 0 ? trimmed.slice(0, cut) : null;
}

/** `grant` is `dir` itself or an ancestor of it (a grant covers its subtree). */
const covers = (grant: string, dir: string): boolean => {
  const base = grant.replace(/\/+$/, '');
  const target = dir.replace(/\/+$/, '');
  return target === base || target.startsWith(`${base}/`);
};

// ---------------------------------------------------- the explanation dialog
//
// A promise resolved by a React component, because the callers are not
// components: the batch start (`Toolbar`) and the per-file start
// (`transcription.ts`) both need an answer synchronously before dispatching.

export interface FolderGrantRequest {
  /** Source folders that need a grant, in the order the queue needs them. */
  dirs: string[];
}

let request: FolderGrantRequest | null = null;
let answerRequest: ((accepted: boolean) => void) | null = null;
const requestListeners = new Set<() => void>();

const notifyRequest = () => {
  for (const listener of requestListeners) listener();
};

/** For `useSyncExternalStore` in `FolderGrantDialog`. */
export const subscribeFolderGrantRequest = (listener: () => void): (() => void) => {
  requestListeners.add(listener);
  return () => requestListeners.delete(listener);
};

/** The request waiting for an answer, or `null`. */
export const getFolderGrantRequest = (): FolderGrantRequest | null => request;

/**
 * Resolve the open request. Both buttons go through here, so the caller's
 * `await` always continues — a dialog that can be dismissed by Escape without
 * answering would leave the queue waiting forever.
 */
export function answerFolderGrant(accepted: boolean): void {
  const resolve = answerRequest;
  request = null;
  answerRequest = null;
  notifyRequest();
  resolve?.(accepted);
}

const askFolderGrant = (dirs: string[]): Promise<boolean> =>
  new Promise<boolean>((resolve) => {
    // A queued dialog would be answered by a click meant for the new one.
    if (answerRequest) answerRequest(false);
    request = { dirs };
    answerRequest = resolve;
    notifyRequest();
  });

// ---------------------------------------------------- the grant itself

let inFlight: Promise<boolean> | null = null;

/**
 * Make sure the app can write where this run's subtitles will go.
 *
 * `same directory` (the default) needs a grant for **each source folder**: the
 * panel opens already pointing at that folder, and once the user confirms it the
 * subtitle is written beside the video as before. Picking a different folder is
 * treated as "put them here instead" — the queue switches to that folder rather
 * than writing somewhere the user did not choose.
 *
 * `custom` needs a single folder, so a still-valid grant short-circuits without
 * any UI; only a dead bookmark asks again.
 *
 * `false` means "the user declined" — the caller must not dispatch, because the
 * write would fail the same way. Builds without the sandbox resolve `true`
 * immediately and never see a dialog.
 */
export function ensureOutputAccess(sourceDirs: string[]): Promise<boolean> {
  // The explanation and the panel are modal; a second caller arriving while the
  // first is mid-flight waits for the same answer instead of stacking a second
  // dialog on top of the first.
  inFlight ??= resolveOutputAccess(sourceDirs).finally(() => {
    inFlight = null;
  });
  return inFlight;
}

async function resolveOutputAccess(sourceDirs: string[]): Promise<boolean> {
  if (!(await bootstrapSandboxFlag())) return true;

  const state = useStore.getState();

  if (state.storageMode === 'custom') {
    if (state.customStoragePath && (await isWritable(state.customStoragePath))) return true;
    return pickFolder(state.customStoragePath || undefined);
  }

  const missing: string[] = [];
  for (const dir of new Set(sourceDirs.filter((dir) => dir.length > 0))) {
    if (!(await isWritable(dir))) missing.push(dir);
  }
  if (missing.length === 0) return true;

  if (!(await askFolderGrant(missing))) return false;

  for (const dir of missing) {
    const selected = await open({
      directory: true,
      multiple: false,
      defaultPath: dir,
      title: i18n.t('grants.panelTitle'),
    });
    if (!selected || typeof selected !== 'string') return false;
    await authorizeDir(selected);

    if (!covers(selected, dir)) {
      // A different folder than the one that was refused: honour the choice and
      // stop asking for the rest of the run — it is one destination for all.
      state.setCustomStoragePath(selected);
      state.setStorageMode('custom');
      return isWritable(selected);
    }
  }

  return true;
}

/** Ask for one folder and remember it (the `custom` mode path). */
async function pickFolder(defaultPath?: string): Promise<boolean> {
  const selected = await open({
    directory: true,
    multiple: false,
    title: i18n.t('grants.panelTitle'),
    defaultPath,
  });
  if (!selected || typeof selected !== 'string') return false;
  if (!(await isWritable(selected))) return false;

  await authorizeDir(selected);
  const state = useStore.getState();
  state.setCustomStoragePath(selected);
  state.setStorageMode('custom');
  return true;
}


