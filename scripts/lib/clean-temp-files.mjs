#!/usr/bin/env node
// Remove build leftovers, keeping source and the fetched runtime binaries.
//
//   node scripts/lib/clean-temp-files.mjs
//
// Mostly a relic of the v1 Python/FunASR packaging (core-engine, PyInstaller
// output) plus the arch-less binary names that predate `<name>-<triple>`. It is
// kept because a checkout that still holds any of it would bundle them through
// Tauri's `resources/**/*` glob.

import fs from 'node:fs';
import path from 'node:path';

import { log } from './log.mjs';
import { projectRoot, binDir, resourcesDir } from './paths.mjs';
import { remove } from './fs.mjs';

log.info('清理临时文件...');

const directories = [
    path.join(projectRoot, 'core-engine', 'build'),
    path.join(projectRoot, 'core-engine', 'dist'),
    path.join(projectRoot, 'core-engine', '__pycache__'),
    path.join(projectRoot, 'core-engine', '.pytest_cache'),
    path.join(projectRoot, 'core-engine', '.ruff_cache'),
    path.join(resourcesDir, 'core-engine'),
];
for (const dir of directories) {
    if (fs.existsSync(dir)) {
        log.step(`  移除 ${dir}`);
        remove(dir);
    }
}

const coreEngine = path.join(projectRoot, 'core-engine');
if (fs.existsSync(coreEngine)) {
    for (const file of fs.readdirSync(coreEngine).filter((name) => name.endsWith('.spec'))) {
        remove(path.join(coreEngine, file));
    }
}

// Pre-`<triple>` names: anything that still uses them is a leftover.
for (const stale of ['ffmpeg', 'ffprobe', 'monosub-engine']) {
    const target = path.join(binDir, stale);
    if (fs.existsSync(target)) {
        log.step(`  移除 bin/${stale}`);
        remove(target);
    }
}

log.success('清理完成！');
log.plain('');
log.plain('保留的文件：');
log.plain('  ✅ core-engine/ (源代码)');
log.plain('  ✅ src-tauri/resources/bin/*-<target-triple> (带架构后缀的二进制)');
