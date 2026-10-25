// Running external tools: cargo, npm/bun, gh, hdiutil, codesign, winapp, tar.
//
// One wrapper so every call site gets the same three things:
//   - a real error when a tool is missing (not ENOENT from spawn)
//   - the child's exit code checked, so a failed step aborts the flow
//   - Windows `.cmd`/`.bat` shims executed through a shell, which is the only
//     way Windows can launch them. `tauri.cmd` is one, and
//     `spawnSync(..., { shell: false })` fails with EINVAL on those.

import { spawnSync } from 'node:child_process';

const WINDOWS = process.platform === 'win32';

/**
 * Central `spawnSync`.
 *
 * On Windows a shell is used **only** for `.cmd`/`.bat` shims. That matters
 * twice over: `shell: true` implies `windowsVerbatimArguments`, so Node adds no
 * quoting at all and cmd.exe re-splits the line — `C:\Program Files\nodejs\
 * node.exe` arrives as `C:\Program` plus a bogus argument and fails with
 * `'C:\Program' is not recognized`. Everything else spawns directly, which has
 * no such problem and no quoting to get wrong.
 */
function spawn(command, args, options) {
    if (!WINDOWS) return spawnSync(command, args, options);

    const useShell = options.shell ?? needsCmdShell(command);
    if (!useShell) {
        return spawnSync(command, args, { ...options, shell: false });
    }
    return spawnSync(shellQuote(command), args.map(shellQuote), { ...options, shell: true });
}

/**
 * True when cmd.exe has to start `command`.
 *
 * Only `.cmd`/`.bat` truly require it, but a bare name (`npm`, `bun`, `gh`) is
 * left to cmd.exe as well: it is the only thing that applies PATHEXT, and
 * without it a bare name that is really an extension-less shim fails with
 * ENOENT. Anything already spelled as a path is spawned directly.
 */
function needsCmdShell(command) {
    if (/\.(cmd|bat)$/i.test(command)) return true;
    return !/[\\/]/.test(command) && !/\.(exe|com)$/i.test(command);
}

/** Quote only when the value would otherwise be split or eaten by cmd.exe. */
function shellQuote(value) {
    const text = String(value);
    if (text === '') return '""';
    if (!/[\s"&|<>^]/.test(text)) return text;
    // Double the backslashes that precede a quote, then escape the quote itself.
    return `"${text.replace(/(\\*)"/g, '$1$1\\"').replace(/(\\+)$/, '$1$1')}"`;
}

/** Run a tool with inherited stdio. Throws when it is missing or fails. */
export function run(command, args = [], options = {}) {
    const result = spawn(command, args, {
        stdio: 'inherit',
        ...options,
    });

    if (result.error) {
        throw new Error(`failed to run ${command}: ${result.error.message}`);
    }
    if (result.status !== 0) {
        throw new Error(`${command} exited with ${describeStatus(result)}`);
    }
    return result;
}

/** `run` without the throw: returns the exit code (127 when it cannot start). */
export function runQuiet(command, args = [], options = {}) {
    const result = spawn(command, args, {
        stdio: 'pipe',
        ...options,
    });
    if (result.error) return 127;
    return result.status ?? 127;
}

/** Run a tool and return its trimmed stdout. Throws on a non-zero exit. */
export function capture(command, args = [], options = {}) {
    const result = spawn(command, args, {
        stdio: ['ignore', 'pipe', 'pipe'],
        encoding: 'utf8',
        ...options,
    });

    if (result.error) {
        throw new Error(`failed to run ${command}: ${result.error.message}`);
    }
    if (result.status !== 0) {
        const stderr = (result.stderr ?? '').trim();
        throw new Error(`${command} exited with ${describeStatus(result)}${stderr ? `: ${stderr}` : ''}`);
    }
    return (result.stdout ?? '').trim();
}

/** Same as `capture` but returns `null` instead of throwing. */
export function tryCapture(command, args = [], options = {}) {
    try {
        return capture(command, args, options);
    } catch {
        return null;
    }
}

/**
 * stdout **and** stderr, merged.
 *
 * `codesign -dv` prints everything it knows to stderr — capturing only stdout
 * returns an empty string and every signature check silently passes, which is the
 * one outcome a verification script must not have.
 */
export function captureMerged(command, args = [], options = {}) {
    const result = spawn(command, args, {
        stdio: ['ignore', 'pipe', 'pipe'],
        encoding: 'utf8',
        ...options,
    });
    if (result.error) throw new Error(`failed to run ${command}: ${result.error.message}`);
    return `${result.stdout ?? ''}${result.stderr ?? ''}`.trim();
}

export function tryCaptureMerged(command, args = [], options = {}) {
    try {
        return captureMerged(command, args, options);
    } catch {
        return null;
    }
}

/**
 * Absolute path to a tool, or null.
 * `where` on Windows (it also matches `.cmd`/`.bat`), `command -v` elsewhere.
 */
export function which(command) {
    const found =
        process.platform === 'win32'
            ? tryCapture('where', [command])
            : tryCapture('command', ['-v', command]);
    if (!found) return null;
    return found.split(/\r?\n/)[0].trim();
}

export function hasTool(command) {
    return which(command) !== null;
}

/** Abort with one clear message when any required tool is missing. */
export function requireTools(commands, hint = '') {
    const missing = commands.filter((command) => !hasTool(command));
    if (missing.length === 0) return;
    const message = `missing required tool(s): ${missing.join(', ')}`;
    throw new Error(hint ? `${message}. ${hint}` : message);
}

function describeStatus(result) {
    if (result.status !== null) return `code ${result.status}`;
    return `signal ${result.signal}`;
}
