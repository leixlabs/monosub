// Host platform detection and the two strings every bundled binary's name is
// built from: the target triple and the executable suffix.
//
// Why this module exists: `process.arch` alone is not enough. x86_64 is what an
// Intel Mac and a Windows PC both report, so a fetch script that only looked at
// the architecture would download a darwin binary onto a Windows machine and
// install it under `x86_64-apple-darwin` — a file nothing can execute and no
// error mentions. Every naming decision goes through here instead.

import { log } from './log.mjs';

/** `macos` | `windows` | `linux` — the names used throughout the scripts. */
export function hostPlatform() {
    switch (process.platform) {
        case 'darwin':
            return 'macos';
        case 'win32':
            return 'windows';
        case 'linux':
            return 'linux';
        default:
            return 'unknown';
    }
}

export function isMacOS() {
    return hostPlatform() === 'macos';
}

export function isWindows() {
    return hostPlatform() === 'windows';
}

/**
 * The triple `engine::cli::target_triple()` produces on the Rust side.
 *
 * The two must agree or the packaging script installs a file the resolver never
 * looks for. It tracks the *build* target, not the host, which is also why the
 * CLI binaries are named from this and not from `rustc -vV`.
 *
 * Only x64 ships on Windows: the CUDA packages upstream publishes are x64-only.
 */
export function targetTriple() {
    const arch = process.arch;
    switch (hostPlatform()) {
        case 'macos':
            if (arch === 'arm64') return 'aarch64-apple-darwin';
            if (arch === 'x64') return 'x86_64-apple-darwin';
            throw new Error(`unsupported macOS architecture: ${arch}`);
        case 'windows':
            if (arch === 'x64') return 'x86_64-pc-windows-msvc';
            throw new Error(
                `unsupported Windows architecture: ${arch} (audio.cpp and llama.cpp ship x64 CUDA builds only)`
            );
        case 'linux':
            if (arch === 'arm64') return 'aarch64-unknown-linux-gnu';
            if (arch === 'x64') return 'x86_64-unknown-linux-gnu';
            throw new Error(`unsupported Linux architecture: ${arch}`);
        default:
            throw new Error(`unsupported platform: ${process.platform}`);
    }
}

/** `.exe` on Windows, empty elsewhere. Every installed binary name ends with it. */
export const exeSuffix = process.platform === 'win32' ? '.exe' : '';

/** `<name>-<triple><exe>` — the file name the resolver and these scripts agree on. */
export function bundledName(binary) {
    return `${binary}-${targetTriple()}${exeSuffix}`;
}

/**
 * `monosub-cli<exe>` — the one bundled binary **without** a target triple.
 *
 * The engines keep the suffix because `engine::cli::resolve_bundled_binary`
 * resolves them and tries both forms. The CLI is different: it is the file the
 * user runs, and the Windows portable instructions put `resources/bin` on
 * `PATH`. There, `monosub-cli-x86_64-pc-windows-msvc.exe` answers no
 * `monosub-cli` lookup, so the name has to be the command.
 *
 * Mirrors `engine::cli::monosub_cli_file_name()`; `verify.mjs` asserts the two
 * agree with what actually landed in the package.
 */
export const cliName = `monosub-cli${exeSuffix}`;

/**
 * Abort when the current host is not one of `allowed`.
 *
 * Used by the handful of steps that genuinely have no cross-platform form
 * (codesigning a DMG, packing an MSIX) so they fail with a pointer at what to
 * run instead of doing something half-right.
 */
export function requireHost(...allowed) {
    const current = hostPlatform();
    if (allowed.includes(current)) return current;

    const list = allowed.join(' / ');
    log.err(`this step runs on ${list} only (current: ${current})`);
    if (!allowed.includes(current) && current === 'windows' && allowed.includes('macos')) {
        log.err('on Windows run the same script there — it branches internally.');
    }
    process.exit(1);
}
