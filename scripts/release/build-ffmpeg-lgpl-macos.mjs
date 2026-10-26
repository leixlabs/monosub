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
import { ensureDir, copyFile, remove, makeScratchDir, fileSize } from '../lib/fs.mjs';
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
    '--enable-runtime-cpudetect',
    '--enable-videotoolbox',
    '--enable-audiotoolbox',
    '--pkg-config-flags=--static',
    '--disable-libx264',
    '--disable-libx265',
    '--disable-libxvid',
    '--disable-libvidstab',
    '--disable-librubberband',
];

async function main() {
    ensureDir(binDir);
    ensureDir(licensesDir);

    const ffmpegTarget = path.join(binDir, bundledName('ffmpeg'));
    const ffprobeTarget = path.join(binDir, bundledName('ffprobe'));
    if (isExecutable(ffmpegTarget) && isExecutable(ffprobeTarget)) {
        verifyFfmpegLicense(ffmpegTarget);
        verifyFfmpegLicense(ffprobeTarget);
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
        run('./configure', [`--prefix=${prefix}`, ...CONFIGURE_FLAGS], { cwd: sourceDir });

        log.step('Compiling (this takes a while) ...');
        const jobs = String(capture('sysctl', ['-n', 'hw.ncpu']).trim() || os.cpus().length);
        run('make', ['-j', jobs], { cwd: sourceDir });
        run('make', ['install'], { cwd: sourceDir });

        copyFile(path.join(prefix, 'bin', 'ffmpeg'), ffmpegTarget);
        copyFile(path.join(prefix, 'bin', 'ffprobe'), ffprobeTarget);

        // Carry the LGPL text into the bundle so the shipped app can satisfy the
        // attribution and written-offer obligations.
        const licenseSource = path.join(sourceDir, 'COPYING.LGPLv2.1');
        if (fs.existsSync(licenseSource)) {
            copyFile(licenseSource, path.join(licensesDir, 'ffmpeg-LICENSE'), 0o644);
            log.ok(`Installed ffmpeg-LICENSE (${humanSize(fileSize(path.join(licensesDir, 'ffmpeg-LICENSE')))})`);
        }

        verifyFfmpegLicense(ffmpegTarget);
        verifyFfmpegLicense(ffprobeTarget);
        log.success(`Built LGPL FFmpeg for ${targetTriple()}`);
    } finally {
        remove(scratch);
    }
}

main().catch((error) => {
    log.err(error.message);
    process.exit(1);
});
