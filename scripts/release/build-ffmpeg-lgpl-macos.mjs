#!/usr/bin/env node
// macOS only: build an LGPL-only FFmpeg for bundling with MonoSub.
//
//   node scripts/release/build-ffmpeg-lgpl-macos.mjs
//
// Why this exists — `brew install ffmpeg` is configured with `--enable-gpl`,
// which puts the *whole* binary under the GPL (FFmpeg's own legal page says so).
// We cannot convey that inside a closed-source app sold through the Mac App
// Store: §6 wants Corresponding Source we never shipped, and §10 forbids the
// further restrictions Apple's EULA imposes. Building GPL-free keeps FFmpeg
// under the LGPL, whose copyleft reaches only FFmpeg itself — not the app that
// runs it as a separate child process.
//
// Nothing MonoSub does needs a GPL codec: audio extraction is `pcm_s16le`,
// thumbnails are PNG/MJPEG, and both are native FFmpeg components, so an LGPL
// build costs no functionality.
//
// Result: resources/bin/ffmpeg-<triple> and resources/bin/ffprobe-<triple>.
// Run prepare-resources.mjs afterwards to refresh MANIFEST.txt.

import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';

import { log, die, humanSize } from '../lib/log.mjs';
import { hostPlatform, targetTriple, bundledName } from '../lib/platform.mjs';
import { binDir, licensesDir } from '../lib/paths.mjs';
import { ensureDir, copyFile, remove, makeScratchDir, fileSize, isExecutable } from '../lib/fs.mjs';
import { download, assertSha256 } from '../lib/net.mjs';
import { run, capture, requireTools } from '../lib/shell.mjs';
import { verifyFfmpegLicense } from '../lib/ffmpeg.mjs';

// macOS only, and deliberately not via requireHost(): its generic hint tells
// Windows users to re-run the same script there, which is wrong for this one —
// the Windows build uses a prebuilt package in prepare-resources.mjs instead.
if (hostPlatform() !== 'macos') {
    die('this script builds the macOS FFmpeg and runs on macOS only.');
}

// Pin the source release and its checksum together; bump both at once.
const FFMPEG_VERSION = '8.1.3';
const FFMPEG_SOURCE_SHA256 = '7138d28c96d9d3e3af4ee3d8cad72741f8ffb40da90c1112235dea3ecd3178a3';
const FFMPEG_SOURCE_URL = `https://ffmpeg.org/releases/ffmpeg-${FFMPEG_VERSION}.tar.xz`;

// The app binary is built for macOS 11 (see `otool -l` on target/release/monosub,
// and tauri.conf.json sets no higher floor), so the bundled ffmpeg has to match.
// Without this the compiler defaults to the *build machine's* OS: building on
// macOS 26 stamps `minos 26.0` into the binary, dyld then refuses to load it on
// macOS 11–15, and the app silently loses thumbnails and audio extraction there
// while ASR (whose audio.cpp build is older) still works.
const MACOS_DEPLOYMENT_TARGET = '11.0';

// `--disable-gpl` / `--disable-nonfree` are the point of this build. The
// explicit `--disable-lib*` entries are belt-and-braces: with GPL off, configure
// already refuses to link them, but naming them documents the boundary and
// protects against someone flipping `--enable-gpl` back on later.
const CONFIGURE_FLAGS = [
    '--disable-gpl',
    '--disable-nonfree',
    '--disable-version3',
    '--enable-static',
    '--disable-shared',
    '--disable-doc',
    '--disable-debug',
    '--disable-ffplay',
    // Without this, configure links every external library pkg-config happens to
    // find — on a developer Mac that is Homebrew's libxcb/libX11, which puts
    // /opt/homebrew paths back into a binary that is supposed to run on a clean
    // machine. Autodetect off keeps the build to FFmpeg's own code plus the
    // frameworks explicitly enabled below.
    '--disable-autodetect',
    // --disable-autodetect drops zlib with everything else; keep it, and note it
    // resolves to the system /usr/lib/libz.1.dylib, not Homebrew's.
    '--enable-zlib',
    '--enable-runtime-cpudetect',
    '--enable-videotoolbox',
    '--enable-audiotoolbox',
    '--pkg-config-flags=--static',
    '--disable-libx264',
    '--disable-libx265',
    '--disable-libxvid',
    '--disable-libvidstab',
    '--disable-librubberband',
    // X11/XCB are only needed by ffplay (disabled) and the xcbgrab input device;
    // naming them keeps the boundary visible if --disable-autodetect is ever
    // dropped by mistake.
    '--disable-libxcb',
    '--disable-libxcb-shm',
    '--disable-libxcb-shape',
    '--disable-libxcb-xfixes',
];

/**
 * Fail when the linked binary still points at something outside the OS.
 *
 * The point of this script is a binary that runs on a machine without Homebrew.
 * `--disable-autodetect` is what keeps pkg-config's findings out, but the check
 * lives here too: a silently Homebrew-dependent ffmpeg passes every other gate,
 * then breaks on a user's machine (or drags /opt/homebrew paths into the app
 * bundle, which notarization rejects).
 *
 * @param {string} binary Path to an ffmpeg or ffprobe executable.
 */
function verifySelfContained(binary) {
    const linked = capture('otool', ['-L', binary])
        .split('\n')
        .map((line) => line.trim().split(' (')[0])
        .filter((entry) => entry && !entry.endsWith(':') && !entry.startsWith('/usr/lib/') && !entry.startsWith('/System/'));

    if (linked.length > 0) {
        die(`${path.basename(binary)} still links ${linked.join(', ')} — the build is not self-contained.`);
    }
}

/**
 * Fail when the binary was stamped for a newer macOS than the app supports.
 *
 * A build on a recent Mac silently records that Mac's version as `minos`, and
 * dyld then refuses to load the binary on anything older — the failure shows up
 * on a user's machine as "no thumbnails, no audio extraction", not as a build
 * error. `MACOSX_DEPLOYMENT_TARGET` above is the fix; this is the proof.
 *
 * @param {string} binary Path to an ffmpeg or ffprobe executable.
 */
function verifyDeploymentTarget(binary) {
    const match = /minos\s+([\d.]+)/.exec(capture('otool', ['-l', binary]));
    if (!match) {
        die(`cannot read the minimum macOS version of ${path.basename(binary)}; refusing to ship a blind build.`);
    }

    const actual = match[1];
    if (compareVersions(actual, MACOS_DEPLOYMENT_TARGET) > 0) {
        die(`${path.basename(binary)} requires macOS ${actual}, above the app's ${MACOS_DEPLOYMENT_TARGET} floor.`);
    }
}

/** Numeric dotted-version comparison: negative when `a` < `b`. */
function compareVersions(a, b) {
    const left = a.split('.').map(Number);
    const right = b.split('.').map(Number);
    for (let i = 0; i < Math.max(left.length, right.length); i += 1) {
        const diff = (left[i] ?? 0) - (right[i] ?? 0);
        if (diff !== 0) return diff;
    }
    return 0;
}

/** Environment every configure/make step runs with. */
function buildEnv() {
    return { ...process.env, MACOSX_DEPLOYMENT_TARGET: MACOS_DEPLOYMENT_TARGET };
}

async function main() {
    ensureDir(binDir);
    ensureDir(licensesDir);

    const ffmpegTarget = path.join(binDir, bundledName('ffmpeg'));
    const ffprobeTarget = path.join(binDir, bundledName('ffprobe'));
    if (isExecutable(ffmpegTarget) && isExecutable(ffprobeTarget)) {
        verifyFfmpegLicense(ffmpegTarget);
        verifyFfmpegLicense(ffprobeTarget);
        verifySelfContained(ffmpegTarget);
        verifySelfContained(ffprobeTarget);
        verifyDeploymentTarget(ffmpegTarget);
        verifyDeploymentTarget(ffprobeTarget);
        log.ok(`LGPL FFmpeg already present, skipping (${targetTriple()})`);
        return;
    }

    // Intel builds need nasm for the x86 asm; Apple Silicon needs nothing extra.
    const needsNasm = process.arch !== 'arm64';
    requireTools(['clang', 'make', 'pkg-config', 'tar', ...(needsNasm ? ['nasm'] : [])], 'xcode-select --install && brew install nasm pkg-config');

    const scratch = makeScratchDir('monosub-ffmpeg-build-');
    const prefix = path.join(scratch, 'prefix');
    try {
        const archive = path.join(scratch, `ffmpeg-${FFMPEG_VERSION}.tar.xz`);
        log.step(`Downloading FFmpeg ${FFMPEG_VERSION} source ...`);
        await download(FFMPEG_SOURCE_URL, archive, { label: `ffmpeg-${FFMPEG_VERSION}.tar.xz` });
        assertSha256(archive, FFMPEG_SOURCE_SHA256, `ffmpeg-${FFMPEG_VERSION}.tar.xz`);

        log.step('Extracting source ...');
        const sourceRoot = path.join(scratch, 'src');
        ensureDir(sourceRoot);
        run('tar', ['-xf', archive, '-C', sourceRoot]);

        const entries = fs.readdirSync(sourceRoot).filter((name) => fs.statSync(path.join(sourceRoot, name)).isDirectory());
        if (entries.length !== 1) die(`unexpected FFmpeg source layout: ${entries.join(', ') || '(empty)'}`);
        const sourceDir = path.join(sourceRoot, entries[0]);

        log.step('Configuring (LGPL only: no --enable-gpl, no --enable-nonfree) ...');
        run('./configure', [`--prefix=${prefix}`, ...CONFIGURE_FLAGS], { cwd: sourceDir, env: buildEnv() });

        log.step('Compiling (this takes a while) ...');
        const jobs = String(capture('sysctl', ['-n', 'hw.ncpu']).trim() || os.cpus().length);
        run('make', ['-j', jobs], { cwd: sourceDir, env: buildEnv() });
        run('make', ['install'], { cwd: sourceDir, env: buildEnv() });

        copyFile(path.join(prefix, 'bin', 'ffmpeg'), ffmpegTarget);
        copyFile(path.join(prefix, 'bin', 'ffprobe'), ffprobeTarget);

        // Carry the LGPL text into the bundle so the shipped app can satisfy the
        // attribution and written-offer obligations. The file is TRACKED, and the
        // Windows flow writes BtbN's LGPLv3 text to the same path — copying
        // unconditionally would flip the committed file between v2.1 and v3 on
        // every macOS build. Until the two platforms get separate license files,
        // keep whatever is committed; a fresh clone already carries the text.
        const licenseSource = path.join(sourceDir, 'COPYING.LGPLv2.1');
        const licenseTarget = path.join(licensesDir, 'ffmpeg-LICENSE');
        if (fs.existsSync(licenseTarget)) {
            log.plain('    ffmpeg-LICENSE 已存在（已入库），保留不覆盖');
        } else if (fs.existsSync(licenseSource)) {
            copyFile(licenseSource, licenseTarget, 0o644);
            log.ok(`Installed ffmpeg-LICENSE (${humanSize(fileSize(path.join(licensesDir, 'ffmpeg-LICENSE')))})`);
        }

        verifyFfmpegLicense(ffmpegTarget);
        verifyFfmpegLicense(ffprobeTarget);
        verifySelfContained(ffmpegTarget);
        verifySelfContained(ffprobeTarget);
        verifyDeploymentTarget(ffmpegTarget);
        verifyDeploymentTarget(ffprobeTarget);
        log.success(`Built LGPL FFmpeg for ${targetTriple()}`);
    } finally {
        remove(scratch);
    }
}

main().catch((error) => {
    log.err(error.message);
    process.exit(1);
});
