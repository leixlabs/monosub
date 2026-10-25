#!/usr/bin/env node
// macOS only: build self-contained ffmpeg/ffprobe binaries.
//
//   node scripts/release/bundle-ffmpeg.mjs
//
// Copies the Homebrew binaries plus every non-system dylib they need into
// resources/bin/lib/, then rewrites all library references to `@loader_path` so
// the result works on a machine without Homebrew.
//
// This is opt-in and separate from prepare-resources.mjs, which only copies the
// binaries. Run it when the copied ffmpeg turns out to depend on
// /opt/homebrew paths. It needs `otool` and `install_name_tool` (Xcode command
// line tools) and re-signs ad-hoc afterwards, because rewriting the load commands
// invalidates the existing signature.

import fs from 'node:fs';
import path from 'node:path';

import { log, die } from '../lib/log.mjs';
import { requireHost, targetTriple, bundledName } from '../lib/platform.mjs';
import { binDir } from '../lib/paths.mjs';
import { ensureDir, remove, copyFile } from '../lib/fs.mjs';
import { run, capture, tryCapture, requireTools } from '../lib/shell.mjs';

requireHost('macos');
requireTools(['otool', 'install_name_tool', 'codesign']);

const triple = targetTriple();
const libDir = path.join(binDir, 'lib');

const ffmpegBin = tryCapture('command', ['-v', 'ffmpeg']);
const ffprobeBin = tryCapture('command', ['-v', 'ffprobe']);
if (!ffmpegBin || !ffprobeBin) die('ffmpeg and ffprobe must be installed (brew install ffmpeg)');

const ffmpegTarget = path.join(binDir, bundledName('ffmpeg'));
const ffprobeTarget = path.join(binDir, bundledName('ffprobe'));

log.info(`Bundling ffmpeg/ffprobe for ${triple}`);
log.plain(`    ffmpeg:  ${ffmpegBin}`);
log.plain(`    ffprobe: ${ffprobeBin}`);

remove(libDir);
ensureDir(binDir);
ensureDir(libDir);

copyFile(ffmpegBin, ffmpegTarget);
copyFile(ffprobeBin, ffprobeTarget);

/** Every non-system dylib `binary` loads, recursing into each one's own deps. */
function collectDylibs(binary) {
    for (const lib of dependentDylibs(binary)) {
        const name = path.basename(lib);
        const destination = path.join(libDir, name);
        if (fs.existsSync(destination)) continue;
        if (!fs.existsSync(lib)) {
            log.warn(`    Warning: ${lib} not found`);
            continue;
        }
        log.plain(`    Copying ${name}`);
        copyFile(lib, destination);
        collectDylibs(destination);
    }
}

function dependentDylibs(file) {
    const output = capture('otool', ['-L', file]);
    return output
        .split('\n')
        .slice(1)
        .map((line) => line.trim().split(/\s+/)[0])
        .filter((lib) => lib && !lib.startsWith('/System/') && !lib.startsWith('/usr/lib/') && !lib.startsWith('@'));
}

/** Rewrite each absolute dependency to `<prefix>/<name>`. */
function fixReferences(file, prefix) {
    for (const lib of dependentDylibs(file)) {
        const name = path.basename(lib);
        tryCapture('install_name_tool', ['-change', lib, `${prefix}/${name}`, file]);
    }
}

log.info('Collecting dynamic libraries...');
collectDylibs(ffmpegTarget);
collectDylibs(ffprobeTarget);

log.info('Rewriting library paths in binaries...');
// The binaries sit in bin/ and load from bin/lib/.
fixReferences(ffmpegTarget, '@loader_path/lib');
fixReferences(ffprobeTarget, '@loader_path/lib');

log.info('Rewriting library paths in dylibs...');
for (const entry of fs.readdirSync(libDir)) {
    if (!entry.endsWith('.dylib')) continue;
    const lib = path.join(libDir, entry);
    tryCapture('install_name_tool', ['-id', `@loader_path/${entry}`, lib]);
    // The dylibs are all in bin/lib/, so they reference each other without a
    // directory component.
    fixReferences(lib, '@loader_path');
}

// Rewriting the load commands invalidates the signatures.
log.info('Re-signing binaries...');
for (const file of [ffmpegTarget, ffprobeTarget, ...fs.readdirSync(libDir).map((name) => path.join(libDir, name))]) {
    tryCapture('codesign', ['--force', '--sign', '-', file]);
}

log.blank();
log.info(`Verifying ffmpeg-${triple} dependencies:`);
log.plain(dependentDylibs(ffmpegTarget).slice(0, 20).join('\n'));

log.blank();
log.info('Testing execution...');
run(ffmpegTarget, ['-version']);
run(ffprobeTarget, ['-version']);

log.blank();
log.success('Done.');
log.plain(`    ${ffmpegTarget}`);
log.plain(`    ${ffprobeTarget}`);
log.plain('    Libraries:');
for (const entry of fs.readdirSync(libDir)) log.plain(`      ${entry}`);
