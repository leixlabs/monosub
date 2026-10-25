// Filesystem helpers shared by every flow.
//
// Small on purpose, but two of them exist for reasons worth recording:
//
//   writeTextFile forces UTF-8 without a BOM. The MSIX manifest is one of the
//   files written here, and appx packaging rejects a BOM-prefixed manifest
//   outright; PowerShell 5.1's `Out-File -Encoding utf8` added one, which is why
//   the Windows scripts had to reach for XmlWriter.
//
//   isExecutable is used to decide whether a fetched binary is already in place.
//   On Windows there is no +x bit, so "is a non-empty file" is the answer —
//   checking the mode there would treat every download as missing and refetch it
//   on every run.

import fs from 'node:fs';
import fsp from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';

export function exists(target) {
    return fs.existsSync(target);
}

export function isFile(target) {
    return fs.existsSync(target) && fs.statSync(target).isFile();
}

export function isExecutable(target) {
    if (!isFile(target)) return false;
    if (process.platform === 'win32') return fs.statSync(target).size > 0;
    // eslint-disable-next-line no-bitwise
    return (fs.statSync(target).mode & 0o111) !== 0;
}

export function ensureDir(dir) {
    fs.mkdirSync(dir, { recursive: true });
    return dir;
}

export function remove(target) {
    fs.rmSync(target, { recursive: true, force: true });
}

/** Copy a file, creating the parent directory. Used for every installed binary. */
export function copyFile(from, to, mode = 0o755) {
    ensureDir(path.dirname(to));
    fs.copyFileSync(from, to);
    if (process.platform !== 'win32') fs.chmodSync(to, mode);
    return to;
}

export function copyDir(from, to) {
    ensureDir(path.dirname(to));
    fs.cpSync(from, to, { recursive: true });
    return to;
}

export function chmod(target, mode) {
    if (process.platform !== 'win32') fs.chmodSync(target, mode);
}

export function fileSize(target) {
    return fs.statSync(target).size;
}

/** Every file under `dir`, as POSIX-style relative paths, sorted. */
export function listFiles(dir, { exclude = [] } = {}) {
    const out = [];
    const walk = (current, prefix) => {
        for (const entry of fs.readdirSync(current, { withFileTypes: true })) {
            const rel = prefix ? `${prefix}/${entry.name}` : entry.name;
            if (exclude.includes(entry.name)) continue;
            if (entry.isDirectory()) {
                walk(path.join(current, entry.name), rel);
            } else if (entry.isFile()) {
                out.push(rel);
            }
        }
    };
    walk(dir, '');
    return out.sort();
}

/** Files directly inside `dir` whose name starts with `prefix`. */
export function filesWithPrefix(dir, prefix) {
    if (!fs.existsSync(dir)) return [];
    return fs
        .readdirSync(dir, { withFileTypes: true })
        .filter((entry) => entry.isFile() && entry.name.startsWith(prefix))
        .map((entry) => path.join(dir, entry.name));
}

export function readTextFile(target) {
    return fs.readFileSync(target, 'utf8');
}

/** UTF-8, no BOM, LF endings — see the header note. */
export function writeTextFile(target, contents) {
    ensureDir(path.dirname(target));
    fs.writeFileSync(target, contents, { encoding: 'utf8' });
    return target;
}

/** A scratch directory outside the repo, so an interrupted download cannot be bundled. */
export function makeScratchDir(prefix = 'monosub-') {
    return fs.mkdtempSync(path.join(os.tmpdir(), prefix));
}

export async function sleep(ms) {
    return new Promise((resolve) => setTimeout(resolve, ms));
}

export { fsp };
