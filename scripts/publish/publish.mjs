#!/usr/bin/env node
// Step 2 of publishing: upload what is in dist/ to GitHub Releases.
//
//   node scripts/publish/publish.mjs                     release everything in dist/
//   node scripts/publish/publish.mjs --platform macos    only dist/*.dmg
//   node scripts/publish/publish.mjs --platform windows  only dist/*.msix
//   node scripts/publish/publish.mjs --version 2.0.0     publish a specific built version
//   node scripts/publish/publish.mjs --recreate          replace an existing release
//   node scripts/publish/publish.mjs --add-assets        only add assets to an existing one
//   node scripts/publish/publish.mjs --skip-bump         do not bump the version afterwards
//   node scripts/publish/publish.mjs --draft             create a draft (drafts do not bump)
//   node scripts/publish/publish.mjs --asset <path>      upload only this file (repeatable)
//   node scripts/publish/publish.mjs --notes-file <file> use the file as release notes
//
// Environment:
//   RELEASE_REPO   target repository, default leixlabs/monosub
//
// Requires the GitHub CLI, already authenticated (`gh auth login`).
//
// Almost none of this is platform-specific — it attaches files to a tag derived
// from a version number — so it is one script. The only platform knowledge is
// which file extensions belong to which platform, and the two "first launch"
// notes that differ between a DMG and an MSIX.

import fs from 'node:fs';
import path from 'node:path';
import { spawnSync } from 'node:child_process';

import { log, die, header, humanSize } from '../lib/log.mjs';
import { projectRoot, distDir } from '../lib/paths.mjs';
import { readTauriVersion, bumpPatchVersionAndSync } from '../lib/version.mjs';
import { runQuiet, hasTool, tryCapture } from '../lib/shell.mjs';

const APP_NAME = 'monosub';
const RELEASE_REPO = process.env.RELEASE_REPO || 'leixlabs/monosub';

const options = parseArgs(process.argv.slice(2));
if (options.help) {
    usage();
    process.exit(0);
}

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
    if (!hasTool('gh')) die('没有找到 gh（GitHub CLI）。安装方式见 https://cli.github.com/');
    if (runQuiet('gh', ['auth', 'status']) !== 0) die('gh 未登录。先执行 gh auth login。');

    if (options.recreate && options.addAssets) {
        die('--recreate 和 --add-assets 是互斥的：一个换产物，一个只补资产。');
    }
    if (options.platform && !['macos', 'windows'].includes(options.platform)) {
        die(`--platform 只接受 macos 或 windows，收到：${options.platform}`);
    }

    header('MonoSub publish — 上传产物到 GitHub Releases');
    log.info(`目标仓库: ${RELEASE_REPO}`);

    // --- Version and tag --------------------------------------------------
    const confVersion = readTauriVersion();
    let version = options.version ?? confVersion;
    // publish follows the *artifact's* version, not the configured one: after the
    // first platform is published the version has already been bumped, so
    // tauri.conf.json is one ahead of dist/ and publishing by config would put the
    // tag and the artifact out of step.
    if (options.version && options.version !== confVersion) {
        log.warn(`按 ${options.version} 发布；tauri.conf.json 里是 ${confVersion}（通常是上一轮发布后自增的下一版）。`);
    }
    if (!/^\d+\.\d+\.\d+$/.test(version)) die(`版本 '${version}' 不是 x.y.z 形式`);

    const tag = `v${version}`;
    const commit = tryCapture('git', ['rev-parse', '--short', 'HEAD']) ?? 'unknown';
    log.info(`版本: ${version}`);
    log.info(`Tag:  ${tag}`);
    log.info(`提交: ${commit}`);

    // --- Collect assets ---------------------------------------------------
    const assets = options.assets.length > 0 ? collectExplicit(options.assets) : collectFromDist(version);

    log.blank();
    log.info(`将上传 ${assets.length} 个文件:`);
    for (const asset of assets) {
        log.plain(`  - ${path.basename(asset)}  (${humanSize(fs.statSync(asset).size)})`);
        // A name without the version in it is usually a stale artifact from an
        // earlier build that got swept in.
        if (!path.basename(asset).includes(version)) {
            log.warn(`    文件名里没有 ${version}，确认一下不是旧产物。`);
        }
    }

    // --- Release notes ----------------------------------------------------
    log.blank();
    let notes;
    if (options.notesFile) {
        notes = readNotesFile(options.notesFile);
    } else {
        log.info('生成 release notes...');
        notes = generateNotes({ version, commit, assets });
    }

    // --- Already released? ------------------------------------------------
    const existing = runQuiet('gh', ['release', 'view', tag, '--repo', RELEASE_REPO]) === 0;
    if (existing) {
        log.blank();
        log.warn(`Release ${tag} 已经存在。`);

        if (options.addAssets) {
            log.info('只补资产（--add-assets）：notes 和版本号都不动');
            upload(tag, assets);
            exitWith(`资产已更新: https://github.com/${RELEASE_REPO}/releases/tag/${tag}`);
        }

        if (!options.recreate) {
            const choice = promptChoice('  [r] 删除后重建（换产物）  [a] 只补资产  [c] 取消: ');
            if (choice === 'c') die('已取消，什么都没改。');
            if (choice === 'a') {
                upload(tag, assets);
                exitWith(`资产已更新: https://github.com/${RELEASE_REPO}/releases/tag/${tag}`);
            }
        }

        log.info(`删除已有的 release ${tag}（不动 tag 本身）...`);
        if (runQuiet('gh', ['release', 'delete', tag, '--repo', RELEASE_REPO, '--yes']) !== 0) {
            die('删除失败，发布中止。');
        }
    }

    // --- Create -----------------------------------------------------------
    // Without a local tag gh creates it at the remote default-branch HEAD, which
    // may not be the commit the artifact was built from.
    if (runQuiet('git', ['rev-parse', '-q', '--verify', `refs/tags/${tag}`]) === 0) {
        log.info(`本地已有 tag ${tag}，release 挂在该提交上`);
    } else {
        log.warn(`本地没有 tag ${tag}：gh 会在远端默认分支 HEAD 上创建它。`);
        log.warn('如果这份产物是从未推送的提交构建的，先 push 再发布。');
    }

    log.blank();
    log.info(`创建 GitHub release ${tag} 并上传资产...`);
    const ghArgs = [
        'release',
        'create',
        tag,
        '--repo',
        RELEASE_REPO,
        '--title',
        `${APP_NAME} ${tag}`,
        '--draft',
        options.draft ? 'true' : 'false',
        '--notes',
        notes,
        ...assets,
    ];

    const result = spawnSync('gh', ghArgs, { stdio: ['ignore', 'pipe', 'pipe'], encoding: 'utf8' });
    if (result.status !== 0) {
        log.err('创建 release 失败');
        log.plain('--- gh 输出 ---');
        log.plain(`${result.stdout ?? ''}${result.stderr ?? ''}`);
        process.exit(1);
    }

    log.blank();
    header(`Release ${tag} 创建成功`);
    log.success(`https://github.com/${RELEASE_REPO}/releases/tag/${tag}`);

    // --- Bump -------------------------------------------------------------
    // The bump has to happen here, after a successful publish: the tag must equal
    // the version compiled into the app, so bumping before the build ships an
    // artifact whose name is one ahead of its tag.
    log.blank();
    if (options.draft) {
        log.info('草稿不自增版本号（草稿还没发布）。');
    } else if (options.skipBump) {
        log.info('跳过版本自增（--skip-bump）。');
    } else {
        log.info('自增版本号，为下次构建做准备...');
        try {
            bumpPatchVersionAndSync();
        } catch (error) {
            log.warn(`版本自增失败（不影响已发布的 release）: ${error.message}`);
        }
    }

    log.blank();
    header('Publish 完成');
}

// ---------------------------------------------------------------------------
// Assets
// ---------------------------------------------------------------------------

function collectExplicit(assets) {
    for (const asset of assets) {
        if (!fs.existsSync(asset)) die(`指定的 --asset 不存在: ${asset}`);
    }
    return assets;
}

function collectFromDist(version) {
    if (!fs.existsSync(distDir)) {
        die(`${distDir} 不存在。先构建：node scripts/release/release.mjs`);
    }

    const isDmg = (name) => name.endsWith('.dmg');
    // `*.signed.msix` is only for local Add-AppxPackage verification: a
    // self-signed package helps nobody else and the Store re-signs anyway.
    const isMsix = (name) => name.endsWith('.msix') && !name.endsWith('.signed.msix');

    let predicate = (name) => isDmg(name) || isMsix(name);
    if (options.platform === 'macos') predicate = isDmg;
    if (options.platform === 'windows') predicate = isMsix;

    const assets = fs
        .readdirSync(distDir, { withFileTypes: true })
        .filter((entry) => entry.isFile() && predicate(entry.name))
        .map((entry) => path.join(distDir, entry.name));

    if (assets.length === 0) {
        die(`${distDir} 里没有可发布的产物。先产出产物：node scripts/release/release.mjs。`);
    }

    // Do not attach last version's artifact under this version's tag: the release
    // page looks completely normal and the download is old code.
    const matched = assets.some((asset) => path.basename(asset).includes(version));
    if (!matched) {
        log.err(`${distDir} 里没有文件名包含 ${version} 的产物 —— 这多半是上一版留下来的。`);
        for (const asset of assets) log.plain(`  - ${path.basename(asset)}`);
        die('用 --version <上面文件名里的版本> 发布那一版，或先重新构建。');
    }
    return assets;
}

function upload(tag, assets) {
    const status = runQuiet('gh', ['release', 'upload', tag, ...assets, '--repo', RELEASE_REPO, '--clobber']);
    if (status !== 0) die('上传失败');
}

// ---------------------------------------------------------------------------
// Release notes
// ---------------------------------------------------------------------------

function readNotesFile(file) {
    if (!fs.existsSync(file)) die(`--notes-file 指向的文件不存在: ${file}`);
    log.info(`Release notes 来自 ${file}`);
    return fs.readFileSync(file, 'utf8');
}

/** The most recent tag that is not the one being published now. */
function lastReleaseTag(tag) {
    const tags = tryCapture('gh', [
        'release',
        'list',
        '--repo',
        RELEASE_REPO,
        '--limit',
        '20',
        '--json',
        'tagName',
        '--jq',
        '.[].tagName',
    ]);
    if (!tags) return '';
    return tags.split(/\r?\n/).map((line) => line.trim()).find((candidate) => candidate && candidate !== tag) ?? '';
}

function generateNotes({ version, commit, assets }) {
    const lastTag = lastReleaseTag(`v${version}`);
    let changes = '';
    if (lastTag && runQuiet('git', ['rev-parse', '-q', '--verify', `refs/tags/${lastTag}`]) === 0) {
        changes = tryCapture('git', ['log', '--pretty=format:- %s', `${lastTag}..HEAD`]) ?? '';
        log.info(`改动范围: ${lastTag}..HEAD`);
    } else {
        changes = tryCapture('git', ['log', '--pretty=format:- %s', '-20']) ?? '';
        if (lastTag) log.warn(`本地没有 ${lastTag} 这个 tag，改动列表退化为最近 20 条提交`);
    }
    if (!changes) changes = '- 与上一个 release 相比没有新提交';

    const rows = assets.map((asset) => {
        const name = path.basename(asset);
        const description = name.endsWith('.dmg')
            ? 'macOS installer (Apple Silicon, ad-hoc signed)'
            : name.endsWith('.msix')
              ? 'MSIX package, Windows 11 x64 (Microsoft Store)'
              : 'attached file';
        return `| \`${name}\` | ${description} |`;
    });

    const hasDmg = assets.some((asset) => asset.endsWith('.dmg'));
    const hasMsix = assets.some((asset) => asset.endsWith('.msix'));

    return [
        "## What's New",
        '',
        `- Release version ${version}`,
        `- Built from commit: \`${commit}\``,
        '',
        '## Changes',
        '',
        changes,
        '',
        '## Download',
        '',
        '| File | Description |',
        '|------|-------------|',
        ...rows,
        '',
        ...(hasDmg ? macOsNotes() : []),
        ...(hasMsix ? windowsNotes() : []),
        '---',
        '',
    ].join('\n');
}

function macOsNotes() {
    return [
        '',
        '## macOS first launch',
        '',
        'This build is **ad-hoc signed** (no Apple Developer ID, not notarized). macOS will',
        'refuse the first launch with "cannot be opened because the developer cannot be',
        'verified". To run it:',
        '',
        '1. Open the .dmg and drag MonoSub to Applications.',
        '2. Launch it once; when the warning appears, click **Done**.',
        '3. Go to **System Settings → Privacy & Security**, scroll down, and click',
        '   **Open Anyway** next to MonoSub.',
        '',
        'This is required once per machine. It is not a sign of a broken download.',
        '',
    ];
}

function windowsNotes() {
    return [
        '',
        '## Windows note',
        '',
        'This MSIX is prepared for Microsoft Store submission; the Store re-signs it on',
        'publish. Installing the file attached here requires trusting a development',
        'certificate, so Windows users should install MonoSub from the Store instead.',
        '',
    ];
}

// ---------------------------------------------------------------------------
// CLI plumbing
// ---------------------------------------------------------------------------

function parseArgs(argv) {
    const options = {
        version: null,
        platform: null,
        assets: [],
        notesFile: null,
        recreate: false,
        addAssets: false,
        skipBump: false,
        draft: false,
        help: false,
    };

    for (let i = 0; i < argv.length; i++) {
        const arg = argv[i];
        const next = () => {
            if (i + 1 >= argv.length) die(`${arg} 需要一个值`);
            return argv[++i];
        };

        if (arg === '--version') options.version = next();
        else if (arg.startsWith('--version=')) options.version = arg.slice('--version='.length);
        else if (arg === '--platform') options.platform = next();
        else if (arg.startsWith('--platform=')) options.platform = arg.slice('--platform='.length);
        else if (arg === '--asset') options.assets.push(next());
        else if (arg.startsWith('--asset=')) options.assets.push(arg.slice('--asset='.length));
        else if (arg === '--notes-file') options.notesFile = next();
        else if (arg.startsWith('--notes-file=')) options.notesFile = arg.slice('--notes-file='.length);
        else if (arg === '--recreate') options.recreate = true;
        else if (arg === '--add-assets') options.addAssets = true;
        else if (arg === '--skip-bump') options.skipBump = true;
        else if (arg === '--draft') options.draft = true;
        else if (arg === '-h' || arg === '--help') options.help = true;
        else die(`未知参数: ${arg}`);
    }
    return options;
}

/**
 * One-key prompt; returns 'r', 'a' or 'c'.
 *
 * Read synchronously from fd 0 in raw mode rather than through `readline`, which
 * is promise/callback based and would force the whole publish flow to become
 * async for the sake of one question.
 */
function promptChoice(question) {
    if (!process.stdin.isTTY) {
        die('非交互环境，无法询问。用 --recreate 换产物，或用 --add-assets 只补资产。');
    }
    process.stdout.write(question);

    const buffer = Buffer.alloc(1);
    const raw = typeof process.stdin.setRawMode === 'function';
    try {
        if (raw) process.stdin.setRawMode(true);
        fs.readSync(0, buffer, 0, 1);
    } finally {
        if (raw) process.stdin.setRawMode(false);
    }
    process.stdout.write('\n');

    const key = buffer.toString('utf8').toLowerCase();
    if (key === 'r') return 'r';
    if (key === 'a') return 'a';
    return 'c';
}

function exitWith(message) {
    log.success(message);
    process.exit(0);
}

function usage() {
    console.log(`Usage: node scripts/publish/publish.mjs [options]

  (无参数)                 发布 dist/ 里当前版本的全部产物（*.dmg + *.msix）
  --version <x.y.z>        指定要发布哪个已构建版本（默认取 tauri.conf.json）
  --platform <macos|windows>  只收该平台的产物（默认两个都收）
  --recreate               已有同名 release 时删除后重建
  --add-assets             已有同名 release 时只上传资产，不改 notes、不自增版本
  --skip-bump              发布成功后不自增版本号
  --draft                  建为草稿；草稿不自增版本号
  --asset <path>           只上传指定文件（可重复，给定时不再扫描 dist/）
  --notes-file <path>      用该文件内容作为 release notes（默认自动生成）
  -h, --help               显示这段说明

本步骤不构建任何东西。先产出产物：node scripts/release/release.mjs`);
}
