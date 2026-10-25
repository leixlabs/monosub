// Version reading, syncing and bumping.
//
// Source of truth: `src-tauri/tauri.conf.json`. Two other files carry the same
// number and are kept in step — `src-tauri/Cargo.toml` (the app reports
// CARGO_PKG_VERSION, which is what the update check compares against the release
// tag) and `package.json`.
//
// The edits are made on the raw text, first occurrence only, rather than by
// parsing and re-serialising: that keeps comments and formatting in Cargo.toml
// and package.json exactly as they are, instead of rewriting the whole file.

import fs from 'node:fs';

import { tauriConfPath, cargoTomlPath, packageJsonPath } from './paths.mjs';
import { log } from './log.mjs';

const SEMVER = /^(\d+)\.(\d+)\.(\d+)$/;

export function readTauriVersion() {
    const text = fs.readFileSync(tauriConfPath, 'utf8');
    const match = text.match(/"version"\s*:\s*"([^"]+)"/);
    if (!match) throw new Error(`could not read the version from ${tauriConfPath}`);
    return match[1];
}

export function validateSemver(version) {
    return SEMVER.test(version);
}

/** Replace the first occurrence of `pattern` — mirrors the old perl one-liners. */
function replaceFirst(file, pattern, replacement) {
    const text = fs.readFileSync(file, 'utf8');
    if (!pattern.test(text)) return false;
    fs.writeFileSync(file, text.replace(pattern, replacement), 'utf8');
    return true;
}

export function setTauriVersion(version) {
    return replaceFirst(tauriConfPath, /"version"\s*:\s*"[^"]+"/, `"version": "${version}"`);
}

export function setCargoVersion(version) {
    return replaceFirst(cargoTomlPath, /^version\s*=\s*"[^"]+"/m, `version = "${version}"`);
}

export function setPackageJsonVersion(version) {
    return replaceFirst(packageJsonPath, /^(\s*)"version"\s*:\s*"[^"]+"/m, `$1"version": "${version}"`);
}

/**
 * Align Cargo.toml and package.json with tauri.conf.json.
 *
 * The build script calls this before packaging so the version inside the app and
 * the version in the artifact's file name cannot disagree — that mismatch is what
 * shipped `MonoSub-2.0.1.dmg` under the tag `v2.0.0` once.
 */
export function syncVersionsFromTauri() {
    const version = readTauriVersion();
    setCargoVersion(version);
    setPackageJsonVersion(version);
    return version;
}

/**
 * Bump the patch component and sync. Returns `"<old> -> <new>"`.
 *
 * Called only after a successful publish. Bumping before the build would make
 * the artifact's version one ahead of the tag the release is created under.
 */
export function bumpPatchVersionAndSync() {
    const current = readTauriVersion();
    const match = current.match(SEMVER);
    if (!match) throw new Error(`unsupported version format: ${current} (expected x.y.z)`);

    const [, major, minor, patch] = match;
    const next = `${major}.${minor}.${Number(patch) + 1}`;

    setTauriVersion(next);
    syncVersionsFromTauri();
    log.ok(`version bumped: ${current} -> ${next}`);
    log.warn('tauri.conf.json, src-tauri/Cargo.toml and package.json changed — commit them.');
    return `${current} -> ${next}`;
}
