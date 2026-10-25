#!/usr/bin/env node
// Step 1 of publishing: produce a publishable artifact on this machine.
//
//   node scripts/release/release.mjs [--skip-build]
//
// It reads the version, builds, and checks what landed in dist/. It does not
// upload and it does not bump the version — that is scripts/publish/publish.mjs.
//
// Why publishing is two steps: a build takes ten-odd minutes, an upload is one
// HTTP call. A failed build must not leave a half-finished release on GitHub, and
// a failed upload (network, permissions, release already exists) must not force a
// rebuild. The two steps meet at dist/ and each can be re-run on its own.
//
// One script for both platforms; the branches differ only in which artifact
// belongs to this platform and what it is called.

import fs from 'node:fs';
import path from 'node:path';

import { log, die, header, humanSize } from '../lib/log.mjs';
import { hostPlatform } from '../lib/platform.mjs';
import { projectRoot, scriptsDir, distDir } from '../lib/paths.mjs';
import { readTauriVersion } from '../lib/version.mjs';
import { run, tryCapture } from '../lib/shell.mjs';

const platform = hostPlatform();
const args = process.argv.slice(2);

if (args.includes('-h') || args.includes('--help')) {
    usage();
    process.exit(0);
}

// `--skip-package` is the name this flag had before the split; kept so old notes
// and habits still work.
const skipBuild = args.includes('--skip-build') || args.includes('--skip-package');

const unknown = args.filter((arg) => !['--skip-build', '--skip-package'].includes(arg));
if (unknown.length > 0) {
    die(`未知参数: ${unknown.join(' ')}`);
}

const artifact =
    platform === 'macos' ? dmgArtifact() : platform === 'windows' ? msixArtifact() : die(`release runs on macOS or Windows (current: ${platform})`);

// `main` is synchronous here; Promise.resolve keeps the .catch valid
// whether it returns a promise or nothing at all.
Promise.resolve()
    .then(main)
    .catch((error) => {
        log.err(error.message);
        process.exit(1);
    });

function main() {
    process.chdir(projectRoot);
    if (!fs.existsSync(path.join(projectRoot, 'package.json')) || !fs.existsSync(path.join(projectRoot, 'src-tauri'))) {
        die(`not the project root: ${projectRoot}`);
    }

    header('MonoSub release — 构建可发布产物');

    const version = readTauriVersion();
    if (!/^\d+\.\d+\.\d+$/.test(version)) {
        die(`tauri.conf.json 里的版本 '${version}' 不是 x.y.z 形式`);
    }

    // The tag must be exactly v<version>: the app's update check compares its own
    // CARGO_PKG_VERSION against the release tag, so any suffix means it never
    // matches and the app offers an update forever.
    const tag = `v${version}`;
    const commit = tryCapture('git', ['rev-parse', '--short', 'HEAD']) ?? 'unknown';

    log.info(`版本: ${version}（release tag 将是 ${tag}）`);
    log.info(`提交: ${commit}`);

    // The artifact comes from the working tree, but the tag gh creates points at
    // the remote HEAD. Warn, do not block: building first and deciding later is
    // legitimate.
    const porcelain = tryCapture('git', ['status', '--porcelain']);
    if (porcelain) {
        log.warn(`工作区有未提交改动：产物里包含这些改动，但 ${tag} 会指向远端 HEAD，`);
        log.warn('不含它们。要发布这次改动，先提交并推送。');
    }

    log.blank();
    if (skipBuild) {
        log.info('跳过构建（--skip-build）：直接使用 dist/ 里已有的产物');
    } else {
        log.info('1/2 构建 + 打包 + 校验（scripts/release/package.mjs）...');
        run(process.execPath, [path.join(scriptsDir, 'release', 'package.mjs')], { cwd: projectRoot });
        log.success('构建完成');
    }

    log.blank();
    log.info(`2/2 核对 ${distDir} 里的产物...`);
    if (!fs.existsSync(distDir)) {
        die(`没有 ${distDir} 目录；先不带 --skip-build 跑一次，或先执行构建。`);
    }

    const found = artifact.collect(distDir);
    if (found.length === 0) {
        die(`${artifact.emptyMessage} (${distDir})`);
    }
    for (const file of found) {
        log.plain(`  - ${file.name}  (${humanSize(file.size)})`);
    }

    const expected = artifact.expectedName(version);
    if (!found.some((file) => file.name === expected)) {
        log.warn(`没有找到预期名称 ${expected} —— 版本号可能与产物不一致。`);
        log.warn('发布时用 node scripts/publish/publish.mjs --version <产物里的版本> 指定。');
    } else {
        log.success(`产物与版本一致: ${expected}`);
    }

    log.blank();
    header('Release 完成：本地产物已就绪（未上传任何东西）');
    log.blank();
    log.plain('下一步 —— 发布到 GitHub Releases:');
    log.plain('  node scripts/publish/publish.mjs');
    log.blank();
    log.plain('  已有同名 release 时的处理方式:');
    log.plain('  node scripts/publish/publish.mjs --recreate     # 删除后重建（换产物）');
    log.plain('  node scripts/publish/publish.mjs --add-assets   # 只补资产，不动 notes');
    log.blank();
}

// ---------------------------------------------------------------------------
// Per-platform artifact rules
// ---------------------------------------------------------------------------

function dmgArtifact() {
    return {
        collect(dir) {
            return listFiles(dir, (name) => name.endsWith('.dmg'));
        },
        expectedName: (version) => `MonoSub-${version}.dmg`,
        emptyMessage: 'dist/ 里没有 .dmg；这次没有可发布的东西',
    };
}

function msixArtifact() {
    return {
        collect(dir) {
            // `*.signed.msix` exists only for local Add-AppxPackage verification: a
            // self-signed package is useless to anyone else and the Store re-signs
            // anyway, so it is not a release artifact.
            return listFiles(dir, (name) => name.endsWith('.msix') && !name.endsWith('.signed.msix'));
        },
        // The MSIX version has four parts where tauri.conf.json has three.
        expectedName: (version) => `monosub_${version}.0_x64.msix`,
        emptyMessage: 'dist/ 里没有可发布的 .msix（signed 副本不算）',
    };
}

function listFiles(dir, predicate) {
    return fs
        .readdirSync(dir, { withFileTypes: true })
        .filter((entry) => entry.isFile() && predicate(entry.name))
        .map((entry) => {
            const full = path.join(dir, entry.name);
            return { name: entry.name, path: full, size: fs.statSync(full).size };
        });
}

function usage() {
    console.log(`Usage: node scripts/release/release.mjs [--skip-build]

  (无参数)          构建 + 打包 + 校验，产物落到 dist/
  --skip-build     跳过构建，只核对 dist/ 里已有的产物
  --skip-package   旧参数名，等价于 --skip-build
  -h, --help       显示这段说明

本步骤不联网、不发布。发布到 GitHub 用 node scripts/publish/publish.mjs。`);
}
