import { invoke } from '@tauri-apps/api/core';

/**
 * The app's own paths, shown in Settings ▸ System.
 *
 * Why this module exists at all: the config file and the models directory are
 * two **unrelated** locations (`src-tauri/src/paths.rs`), and users routinely
 * conflate them. Measured on a real install the settings file lives in
 * `~/Library/Application Support/leixlabs.monosub/settings.json` while the
 * models live wherever that file's `modelsDir` points — here a directory inside
 * the source checkout. Showing the config file next to the models row is what
 * makes the difference visible instead of a claim in a hint.
 *
 * The command-line tool is described from the backend for the same reason.
 * It used to be a hardcoded macOS `ln -s` pointing at `Contents/MacOS/…`, which
 * is the `externalBin` layout the design sketched and no build implements — the
 * bundled CLI ships in `resources/bin/`. A string in the frontend cannot stay
 * true across platforms, so the facts come from the resolvers the app uses.
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

/** Platform-dependent description of the bundled command-line tool. */
export interface CliInfo {
  /** Absolute path of the bundled binary; `null` when this build ships none. */
  path: string | null;
  /** Absolute `resources/bin` directory the binary lives in, when it exists. */
  binDir: string | null;
  /** The binary's file name, `<name>-<target-triple>` (+ `.exe` on Windows). */
  fileName: string;
  /** `windows` / `macos` / `linux`. */
  platform: string;
  /** Windows: the package registers an execution alias, so `PATH` needs nothing. */
  usesExecutionAlias: boolean;
}

/**
 * Ask the backend where the CLI actually is.
 *
 * A build without the binary is a normal answer (`path: null`), not an error:
 * the Microsoft Store package deliberately ships a single entry point, and the
 * card has to be able to say so instead of showing a command that cannot work.
 */
export const getCliInfo = (): Promise<CliInfo> => invoke<CliInfo>('cli_info');

/**
 * Quote one argument for the platform's shell.
 *
 * The path is interpolated into a command the user copies, and an install
 * directory such as `C:\Program Files\MonoSub` contains a space. Single quotes
 * are the POSIX quoting (and an embedded `'` becomes `'\''`); PowerShell takes
 * single quotes too, but escapes an embedded one by doubling it.
 */
const quoteArg = (value: string, platform: string): string => {
  if (platform === 'macos' || platform === 'linux') {
    return `'${value.replace(/'/g, `'\\''`)}'`;
  }
  return `'${value.replace(/'/g, "''")}'`;
};

/**
 * The command that puts `monosub-cli` on `PATH`, for this platform.
 *
 * Returns `null` when no command is needed:
 *
 * * Windows — the package declares an execution alias, so the command already
 *   resolves in a new terminal. `%LOCALAPPDATA%\Microsoft\WindowsApps` is on
 *   `PATH` by default and Windows re-points the alias on every update, which a
 *   `PATH` entry naming the install directory would not survive (MSIX installs
 *   under a version-stamped folder).
 * * A build that ships no CLI at all.
 *
 * The macOS form links into `/usr/local/bin`, which exists on every Mac and is
 * on the default `PATH`; the Windows form explains the one-line PowerShell that
 * appends a directory to the *user* `PATH` for a self-built/portable copy.
 */
export const cliInstallCommand = (info: CliInfo): string | null => {
  if (info.usesExecutionAlias) {
    return null;
  }

  if (!info.path) {
    return null;
  }

  if (info.platform === 'macos' || info.platform === 'linux') {
    return `ln -sf ${quoteArg(info.path, info.platform)} /usr/local/bin/monosub-cli`;
  }

  // Windows self-built / portable copy: append the directory holding the
  // binary to the user PATH. Deliberately registry-preserving — `setx PATH
  // "%PATH%;<dir>"` merges the machine PATH into the user one, truncates past
  // 1024 characters, and writes the value back as a plain string, which
  // permanently stops `%VAR%` entries from expanding. Reading the raw value and
  // writing it back with its own value kind keeps both.
  const dir = info.binDir ?? '';
  const dirArg = quoteArg(dir, info.platform);
  const key = '$k=[Microsoft.Win32.Registry]::CurrentUser.OpenSubKey("Environment",$true)';
  const parts =
    '$v=[string]$k.GetValue("Path","",[Microsoft.Win32.RegistryValueOptions]::DoNotExpandEnvironmentNames);' +
    '$p=@($v -split ";"|Where-Object{$_})';

  return (
    `${key};${parts};` +
    `if($p -notcontains ${dirArg}){` +
    `$k.SetValue("Path",(@($p+${dirArg}) -join ";"),$k.GetValueKind("Path"))};$k.Close()`
  );
};

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
