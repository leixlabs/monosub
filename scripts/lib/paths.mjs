// Every path the build, release and publish flows use, resolved once.
//
// The root is found by walking up for `package.json` + `src-tauri` rather than
// by counting `../`: the scripts moved several times during this refactor, and a
// depth-based walk would have to be edited in every file each time.

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const here = path.dirname(fileURLToPath(import.meta.url)); // <root>/scripts/lib

export function findProjectRoot(from = here) {
    let dir = path.resolve(from);
    for (let i = 0; i < 8; i++) {
        if (
            fs.existsSync(path.join(dir, 'package.json')) &&
            fs.existsSync(path.join(dir, 'src-tauri'))
        ) {
            return dir;
        }
        const parent = path.dirname(dir);
        if (parent === dir) break;
        dir = parent;
    }
    throw new Error(`project root not found above ${from} (looked for package.json + src-tauri)`);
}

export const projectRoot = findProjectRoot();

export const scriptsDir = path.join(projectRoot, 'scripts');
export const srcTauriDir = path.join(projectRoot, 'src-tauri');

/** Bundled runtime resources: what `resources/** /*` puts inside the app. */
export const resourcesDir = path.join(srcTauriDir, 'resources');
export const binDir = path.join(resourcesDir, 'bin');
export const modelsDir = path.join(resourcesDir, 'models');
export const sileroDir = path.join(modelsDir, 'silero_vad');
export const licensesDir = path.join(resourcesDir, 'licenses');
export const manifestPath = path.join(resourcesDir, 'MANIFEST.txt');

/** Release artifacts land here; the publish flow reads only from here. */
export const distDir = path.join(projectRoot, 'dist');

/**
 * MSIX staging. Deliberately NOT under dist/: Vite writes the frontend there and
 * the publish flow collects artifacts from there, so staging inside it would
 * publish the staging tree.
 */
export const stagingDir = path.join(projectRoot, 'build', 'msix');
export const stagingAppDir = path.join(stagingDir, 'app');

/**
 * Dev signing certificate, deliberately OUTSIDE `stagingDir`.
 *
 * Staging is wiped on every build, so a certificate kept there was regenerated
 * every run — and a regenerated certificate silently invalidates the `.cer` the
 * developer already imported into the Root store, which is exactly what makes
 * `Add-AppxPackage` fail with 0x800B010A on the next install.
 */
export const devCertPfx = path.join(projectRoot, 'build', 'devcert.pfx');
export const devCertCer = path.join(projectRoot, 'build', 'devcert.cer');

export const tauriConfPath = path.join(srcTauriDir, 'tauri.conf.json');
export const cargoTomlPath = path.join(srcTauriDir, 'Cargo.toml');
export const packageJsonPath = path.join(projectRoot, 'package.json');
export const iconPath = path.join(srcTauriDir, 'icons', 'icon.png');

/**
 * The Tauri CLI from node_modules, not a global `cargo tauri`: the local copy is
 * pinned to the same version as the tauri crate, so bundling behaviour matches
 * what Cargo.lock resolves.
 *
 * On Windows the shim is `tauri.cmd` — the extensionless `tauri` is a POSIX
 * shell script that Windows cannot execute.
 */
export function tauriCli() {
    const name = process.platform === 'win32' ? 'tauri.cmd' : 'tauri';
    const cli = path.join(projectRoot, 'node_modules', '.bin', name);
    if (!fs.existsSync(cli)) {
        throw new Error(`missing ${cli} — run the package manager install first`);
    }
    return cli;
}
