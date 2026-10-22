import { invoke } from '@tauri-apps/api/core';

/**
 * The two halves of Settings ▸ System that are about *the app's own files*.
 *
 * Why this module exists at all: the config file and the models directory are
 * two **unrelated** locations (`src-tauri/src/paths.rs`), and users routinely
 * conflate them. Measured on a real install the settings file lives in
 * `~/Library/Application Support/dev.leix.monosub/settings.json` while the
 * models live wherever that file's `modelsDir` points — here a directory inside
 * the source checkout. Showing the config file next to the models row is what
 * makes the difference visible instead of a claim in a hint.
 *
 * The CLI snippet is here for the same reason: it is the only way a user gets
 * `monosub-cli` onto `PATH` (the app may not modify `PATH`), so it is a fixed,
 * copyable string rather than something read from the backend.
 */

/**
 * Absolute path of `settings.json`.
 *
 * Returned even when the file does not exist yet: a fresh install writes
 * nothing until the first setting changes, and the path is exactly what the
 * user wants to know in that state.
 */
export const getConfigFilePath = (): Promise<string> => invoke<string>('config_file_path');

/**
 * Show `settings.json` in the system file manager.
 *
 * Resolves to the path actually opened — the file itself, or its directory when
 * the file has not been written yet (the Rust side decides, so the frontend
 * never has to guess).
 */
export const revealConfigFile = (): Promise<string> => invoke<string>('reveal_config_file');

/**
 * The command that puts the bundled CLI on `PATH`.
 *
 * Fixed text, not fetched from the backend: this is the *shipped* macOS layout
 * (`Contents/MacOS/monosub-cli`, per `docs/tech-spec/cli-design.md` §7.2), and a
 * failed IPC call must not be able to blank the one line the user copies.
 */
export const CLI_INSTALL_COMMAND =
  'ln -s /Applications/MonoSub.app/Contents/MacOS/monosub-cli /usr/local/bin/monosub-cli';

/**
 * Put `text` on the clipboard.
 *
 * `navigator.clipboard` is the primary path, but the WebView is not always a
 * secure context (`tauri://localhost` on macOS), where the API is missing or
 * rejects with `NotAllowedError`. The deprecated `execCommand` selection copy is
 * the only fallback available without adding the clipboard *write* permission.
 * This is the same implementation the error dialog uses; it is duplicated here
 * rather than exported from there because that dialog is a component, not a
 * helper module.
 */
export const writeClipboard = async (text: string): Promise<void> => {
  try {
    if (navigator.clipboard?.writeText) {
      await navigator.clipboard.writeText(text);
      return;
    }
  } catch {
    // Fall through to the selection copy below.
  }

  const area = document.createElement('textarea');
  area.value = text;
  area.setAttribute('readonly', '');
  area.style.position = 'fixed';
  area.style.top = '0';
  area.style.opacity = '0';
  document.body.appendChild(area);
  try {
    area.select();
    if (!document.execCommand('copy')) {
      throw new Error('document.execCommand("copy") returned false');
    }
  } finally {
    document.body.removeChild(area);
  }
};
