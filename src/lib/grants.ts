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
