#!/usr/bin/env node
// Install the NSIS artifact this repo just built, from a path Windows will
// actually let it write from.
//
//   node scripts/release/install.mjs                 # build if needed, then install
//   node scripts/release/install.mjs --no-build      # use dist\ as it is
//   node scripts/release/install.mjs --passive       # show the progress page
//   node scripts/release/install.mjs --uninstall     # remove the installed app
//
// ## Why this is not just "double-click dist\...setup.exe"
//
// A Windows process inherits the **mandatory integrity level** of the file it is
// started from. When the project tree carries a Low label, an installer launched
// from `dist\` runs at Low, and Low is not allowed to write %LOCALAPPDATA%,
// %APPDATA% or HKCU. Measured on LEI-PC (2026-11-16) with exactly that setup:
//
//   * GUI: the wizard opened, printed "Output folder:
//     C:\Users\<user>\AppData\Local\MonoSub", and never created it. It sat
//     there; every file step had failed.
//   * `/S`: exit code **0 in 3 ms**, nothing written — no directory, no
//     registry key, no shortcut, no message.
//
// The artifact was fine. The same bytes, copied to %TEMP% first, installed 84
// files / 2025.6 MB in 78 s and registered the shortcut and uninstall entry. An
// A/B of one probe installer (writes to %LOCALAPPDATA% + HKCU) went
// FAILED/FAILED -> ok/ok purely by moving the file, which is what proves the
// label rather than the binary is the variable.
//
// So this script copies the installer to %TEMP% before running it. That is not a
// workaround for a broken build — it is the only way to launch a Medium-integrity
// process from a tree whose label is out of this build's control (a sandbox, a
// group policy, another tool). `check-integrity.mjs --fix` is the other way out,
// and this script says so when it detects the condition.
//
// ## What "installed" has to mean
//
// The NSIS installer exits 0 whether or not it wrote anything (that is the whole
// trap above), so the exit code cannot be the verdict. This checks the file tree,
// the uninstall registry key, and the Start-menu shortcut — and fails loudly
// listing which of them is missing.

import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';

import { log, die, header, humanSize } from '../lib/log.mjs';
import { hostPlatform } from '../lib/platform.mjs';
import { projectRoot, distDir } from '../lib/paths.mjs';
import { exists, isFile, fileSize, ensureDir, copyFile, remove } from '../lib/fs.mjs';
import { run, tryCapture } from '../lib/shell.mjs';
import { syncVersionsFromTauri } from '../lib/version.mjs';

const args = process.argv.slice(2);

if (args.includes('-h') || args.includes('--help')) {
    console.log(`Usage: node scripts/release/install.mjs [--no-build] [--passive] [--uninstall]

  Windows only. Installs the NSIS .exe from dist\\ to %LOCALAPPDATA%\\MonoSub.

  (no args)     build with release:exe when dist\\ has no installer, then install
  --no-build    use whatever dist\\ already holds
  --passive     run the installer with /P (progress window) instead of /S
  --uninstall   run the installed uninstall.exe and stop`);
    process.exit(0);
}

const noBuild = args.includes('--no-build');
const passive = args.includes('--passive');
const uninstall = args.includes('--uninstall');
const unknown = args.filter((arg) => !['--no-build', '--passive', '--uninstall'].includes(arg));
if (unknown.length > 0) die(`未知参数: ${unknown.join(' ')}`);

if (hostPlatform() !== 'windows') {
    die(`release:install 只在 Windows 上运行（当前: ${hostPlatform()}）。`);
}

const installDir = path.join(process.env.LOCALAPPDATA ?? '', 'MonoSub');

Promise.resolve()
    .then(main)
    .catch((error) => {
        log.err(error.message);
        process.exit(1);
    });

function main() {
    header('MonoSub 本机安装（NSIS → %LOCALAPPDATA%\\MonoSub）');

    if (uninstall) return uninstallExisting();

    const installer = findInstaller();
    if (!installer && noBuild) {
        die(`dist\\ 里没有安装包，而 --no-build 禁止构建。先跑 bun run release:exe。`);
    }
    if (!installer) {
        log.step('dist\\ 里没有安装包 —— 先构建（bun run release:exe，最慢的一步是 LZMA）...');
        run(process.execPath, [path.join(projectRoot, 'scripts', 'release', 'installer.mjs')], {
            cwd: projectRoot,
        });
    }

    const built = findInstaller();
    if (!built) die('构建完成但 dist\\ 里仍然没有 *.exe。');

    const version = syncVersionsFromTauri();
    log.ok(`安装包: ${path.basename(built)}  (${humanSize(fileSize(built))})，版本 ${version}`);

    // The whole point: a process inherits the label of the file it starts from,
    // and %TEMP% is Medium even when the project tree is not.
    const staging = path.join(os.tmpdir(), `monosub-install-${Date.now()}`);
    ensureDir(staging);
    const staged = path.join(staging, path.basename(built));
    log.step('复制到干净路径（%TEMP%）后运行 —— 见本文件顶部对完整性级别的说明...');
    copyFile(built, staged);
    log.ok(`临时副本: ${staged}`);

    const label = integrityLabel(projectRoot);
    if (label && !['Medium', 'High', 'System'].includes(label)) {
        log.warn(`项目目录的完整性级别是 ${label}。正在用 %TEMP% 里的副本绕开它。`);
        log.warn('  要根治: node scripts/release/check-integrity.mjs --fix');
    }

    try {
        runInstaller(staged, passive ? '/P' : '/S');
    } finally {
        remove(staging);
    }

    verifyInstalled(version);
}

function runInstaller(installer, mode) {
    log.step(`运行安装程序（${mode}）...`);
    // Inherited stdio so the installer's own DetailPrint output is visible.
    run(installer, [mode]);
    log.ok('安装程序已退出。');
}

/**
 * The verdict is the file system, not the exit code.
 *
 * NSIS exits 0 after a completely failed install (measured), so every claim this
 * script makes is checked here: the tree, the uninstall key, the shortcut.
 */
function verifyInstalled(version) {
    log.step('校验安装结果...');
    const problems = [];

    const exe = path.join(installDir, 'monosub.exe');
    if (!isFile(exe)) {
        problems.push(`缺少主程序: ${exe}`);
    } else {
        const resources = path.join(installDir, 'resources');
        if (!exists(resources)) {
            problems.push(`缺少 resources\\: ${resources}`);
        } else {
            const tree = measure(installDir);
            log.ok(`安装目录: ${installDir}`);
            log.ok(`  ${tree.files} 个文件, ${humanSize(tree.bytes)}`);
        }
    }

    const cli = path.join(installDir, 'monosub-cli.exe');
    if (!isFile(cli)) {
        // Not fatal: the app runs without it, and installer.mjs warns when the
        // CLI source is absent. Worth a line because a silent miss is how the
        // two-binaries-copied-nowhere trap got noticed.
        log.warn('monosub-cli.exe 不在安装目录里（安装包可能没带 CLI）。');
    }

    const reg = uninstallEntry();
    if (!reg) {
        problems.push('没有写入卸载注册项（设置 → 应用 里不会出现 MonoSub）');
    } else {
        log.ok(`卸载项: ${reg.displayName} ${reg.displayVersion} — ${reg.installLocation}`);
        if (reg.displayVersion !== version) {
            problems.push(`卸载项版本是 ${reg.displayVersion}，期望 ${version}`);
        }
    }

    const shortcut = startMenuShortcut();
    if (!shortcut) {
        problems.push('没有开始菜单快捷方式');
    } else {
        log.ok(`开始菜单: ${shortcut}`);
    }

    log.blank();
    if (problems.length > 0) {
        log.err('安装不完整 —— 安装程序退出码为 0 也不代表写成功了:');
        for (const problem of problems) log.plain(`  - ${problem}`);
        log.blank();
        log.plain('若主程序目录根本没建出来，几乎一定是完整性级别的问题：');
        log.plain('  node scripts/release/check-integrity.mjs          # 报告');
        log.plain('  node scripts/release/check-integrity.mjs --fix    # 修复');
        process.exit(1);
    }

    log.success('MonoSub 已安装并注册。');
    log.blank();
    log.plain(`  启动  ${path.join(installDir, 'monosub.exe')}`);
    log.plain('  卸载  bun run release:install --uninstall');
    log.plain('        或 设置 → 应用 → MonoSub → 卸载');
}

function uninstallExisting() {
    const uninstaller = path.join(installDir, 'uninstall.exe');
    if (!isFile(uninstaller)) {
        log.warn(`${uninstaller} 不存在 —— 没装，或已经卸载过。`);
        return;
    }

    // A running monosub.exe holds files in $INSTDIR open, and the NSIS uninstaller
    // leaves the tree behind when it cannot delete them (exit 0, files still there —
    // measured). The uninstaller's own CheckIfAppIsRunning is UI-driven and silent
    // mode suppresses the prompt, so close the app here instead of relying on it.
    stopRunningApp();

    log.step('运行卸载程序（/S）...');
    // The uninstaller lives under %LOCALAPPDATA%, so it is Medium already; no
    // staging needed here.
    //
    // NSIS uninstallers copy themselves to %TEMP%\~nsu*.tmp\Un.exe and relaunch, so
    // the process we start returns in well under a second while the *copy* is still
    // deleting the tree asynchronously. Checking the directory right after the
    // launcher exits therefore always finds it intact (measured: exit 0 in 0.5 s,
    // 84 files still present, then gone a moment later). Wait for the tree to
    // disappear instead of trusting the exit code.
    run(uninstaller, ['/S']);
    const deadline = Date.now() + 120_000;
    while (Date.now() < deadline && exists(installDir)) {
        sleep(250);
    }

    if (exists(installDir)) {
        const left = measure(installDir);
        log.warn(`卸载程序退出后 ${installDir} 仍在（${left.files} 个文件）。`);
        log.warn('  被占用的文件挡着。再试一次，或手动关掉托盘里的 MonoSub。');
        process.exit(1);
    }
    log.success('已卸载。');
}

/** Stop `monosub.exe` / `monosub-cli.exe` so the uninstaller can delete its tree. */
function stopRunningApp() {
    // taskkill prints an error when the process is not running; that is expected and
    // why the result is discarded. /T also takes its WebView2 children with it.
    tryCapture('taskkill', ['/F', '/IM', 'monosub.exe', '/T']);
    tryCapture('taskkill', ['/F', '/IM', 'monosub-cli.exe', '/T']);
    // Give Windows a moment to release the file handles.
    const deadline = Date.now() + 5000;
    while (Date.now() < deadline && processRunning('monosub.exe')) {
        sleep(200);
    }
    if (processRunning('monosub.exe')) {
        log.warn('monosub.exe 仍在运行 —— 卸载可能删不干净。');
    }
}

function processRunning(image) {
    const out = tryCapture('tasklist', ['/FI', `IMAGENAME eq ${image}`, '/NH']);
    return Boolean(out && out.toLowerCase().includes(image.toLowerCase()));
}

/** Block for `ms` without spinning a core — a small wait, used only while a file
 *  handle is released or an asynchronous uninstaller finishes.
 *  `Atomics.wait` is the only synchronous sleep Node offers. */
function sleep(ms) {
    Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, ms);
}

// ---------------------------------------------------------------------------

/** The newest `*.exe` in dist\, or null. */
function findInstaller() {
    if (!exists(distDir)) return null;
    let best = null;
    let bestTime = 0;
    for (const entry of fs.readdirSync(distDir, { withFileTypes: true })) {
        if (!entry.isFile() || !entry.name.toLowerCase().endsWith('.exe')) continue;
        const full = path.join(distDir, entry.name);
        const time = fs.statSync(full).mtimeMs;
        if (time > bestTime) {
            best = full;
            bestTime = time;
        }
    }
    return best;
}

function measure(dir) {
    let files = 0;
    let bytes = 0;
    const walk = (current) => {
        for (const entry of fs.readdirSync(current, { withFileTypes: true })) {
            const full = path.join(current, entry.name);
            if (entry.isDirectory()) walk(full);
            else if (entry.isFile()) {
                files += 1;
                bytes += fs.statSync(full).size;
            }
        }
    };
    walk(dir);
    return { files, bytes };
}

/** The HKCU uninstall entry NSIS writes, or null. */
function uninstallEntry() {
    const key = 'HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\MonoSub';
    const output = tryCapture('reg', ['query', key]);
    if (!output) return null;
    const read = (name) => {
        const match = output.match(new RegExp(`${name}\\s+REG_\\w+\\s+(.+)`));
        return match ? match[1].trim().replace(/^"|"$/g, '') : '';
    };
    return {
        displayName: read('DisplayName'),
        displayVersion: read('DisplayVersion'),
        installLocation: read('InstallLocation'),
    };
}

function startMenuShortcut() {
    const dir = path.join(process.env.APPDATA ?? '', 'Microsoft', 'Windows', 'Start Menu', 'Programs');
    if (!exists(dir)) return null;
    for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
        if (entry.isFile() && /monosub/i.test(entry.name) && entry.name.endsWith('.lnk')) {
            return path.join(dir, entry.name);
        }
    }
    return null;
}

/** The mandatory integrity label of `dir`, or null when unlabelled. */
function integrityLabel(dir) {
    const text = tryCapture('icacls', [dir]);
    if (!text) return null;
    const match = text.match(/Mandatory Label\\(\w+) Mandatory Level/);
    return match ? match[1] : null;
}
