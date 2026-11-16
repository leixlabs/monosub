#!/usr/bin/env node
// Build a double-click Windows installer (.exe) for local use.
//
//   node scripts/release/installer.mjs [--skip-resources]
//
//   Windows → dist\<productName>_<version>_x64-setup.exe   (NSIS, unsigned)
//
// Three Windows artifacts now exist and they are not interchangeable:
//
//   MSIX (release/package.mjs)   Store submission. Needs a certificate in the
//                                Root store to install locally, is re-signed by
//                                the Store, and installs under the version-
//                                stamped WindowsApps folder.
//   NSIS (this script)           Day-to-day self-install. One .exe, no
//                                certificate, no PowerShell, per-user install
//                                under %LOCALAPPDATA%\MonoSub, Start-menu
//                                shortcut and an entry in Settings → Apps.
//   DMG  (release/package.mjs)   macOS.
//
// The MSIX flow deliberately runs `tauri build --no-bundle` because the Store
// payload is assembled by hand afterwards. This one does the opposite: it lets
// Tauri's own NSIS bundler do the work, which is the only way to get the
// installer script, shortcuts, uninstaller and Apps-and-Features registration.
//
// ## The one hard limit
//
// NSIS is 32-bit and cannot emit an installer with a data section over ~2 GB;
// `makensis` fails with "error mmapping file ... is out of range" (or, worse,
// writes a 100 KB stub). With both CUDA engines bundled, `resources\` alone is
// about 2.0 GB uncompressed — 2 GB is the *compressed* output ceiling, and LZMA
// measurably lands it at ~1.31 GiB, so the build fits with room to spare.
// `reportPayload()` prints the real numbers every run and the post-build check
// refuses to copy out an installer that is implausibly small (an empty stub is
// the failure mode above).

import fs from 'node:fs';
import path from 'node:path';

import { log, die, header, humanSize } from '../lib/log.mjs';
import { hostPlatform, cliName } from '../lib/platform.mjs';
import { projectRoot, scriptsDir, srcTauriDir, distDir, resourcesDir, binDir, tauriCli } from '../lib/paths.mjs';
import { ensureDir, copyFile, remove, exists, isFile, listFiles, fileSize } from '../lib/fs.mjs';
import { run, requireTools, tryCapture } from '../lib/shell.mjs';
import { syncVersionsFromTauri } from '../lib/version.mjs';
import { checkedInVersion, installedTauriCliVersion } from '../lib/nsis-template.mjs';

const args = process.argv.slice(2);

if (args.includes('-h') || args.includes('--help')) {
    usage();
    process.exit(0);
}

const skipResources = args.includes('--skip-resources');
const unknown = args.filter((arg) => arg !== '--skip-resources');
if (unknown.length > 0) die(`未知参数: ${unknown.join(' ')}`);

// `main` is synchronous here; Promise.resolve keeps the .catch valid whether it
// returns a promise or nothing at all.
Promise.resolve()
    .then(main)
    .catch((error) => {
        log.err(error.message);
        process.exit(1);
    });

function main() {
    if (hostPlatform() !== 'windows') {
        die(
            `release:exe 只在 Windows 上运行（当前: ${hostPlatform()}）。\n` +
                '  macOS 用 bun run release:package 出 DMG；商店包用 bun run release:package -- --sign 出 MSIX。'
        );
    }

    process.chdir(projectRoot);

    // bun is not optional: tauri.conf.json's beforeBuildCommand is literally
    // `bun run build`, so without it `tauri build` fails inside the frontend step
    // with an error that points at the wrong place.
    requireTools(['cargo', 'bun'], '这两个工具是编译 Rust 与前端所必需的。');

    header('MonoSub 自用安装包 — Windows NSIS (.exe)');

    log.step('1. 清理上一次的安装包...');
    removeOldInstallers();
    const version = syncVersionsFromTauri();
    log.ok(`版本: ${version}（安装包按此版本命名）`);

    log.step('2. 准备 Windows 运行时二进制（CUDA 引擎 / ffmpeg / yt-dlp）...');
    if (skipResources) {
        log.warn('--skip-resources：复用已有的 resources\\，不重新拉取');
    } else {
        runNode(path.join(scriptsDir, 'release', 'prepare-resources.mjs'));
    }

    log.step('3. 安装前端依赖...');
    run('bun', ['install']);

    // Must run BEFORE `tauri build`: the bundler copies resources\ at build time,
    // so a CLI staged afterwards would never reach the installer. (The MSIX flow
    // can copy it later because it assembles the payload itself.)
    log.step('4. 编译命令行工具 monosub-cli.exe...');
    buildCli();

    reportPayload();
    warnAboutViteWipe();
    checkNsisTemplate();
    checkIntegrityLevel();

    log.step('5. 编译并打包 NSIS 安装包（最慢的一步：LZMA 要压 2 GB 资源，本机实测约 6 分钟）...');
    // --bundles nsis pins the target: tauri.conf.json says "all", which would also
    // drag in WiX/MSI — a second multi-GB payload nobody asked for, plus a WiX
    // download on a machine that has never built an MSI.
    run(tauriCli(), ['build', '--bundles', 'nsis']);

    log.step('6. 收集安装包到 dist\\ ...');
    const installer = findInstaller();
    if (!installer) {
        die(
            '没有找到 NSIS 产物（src-tauri\\target\\release\\bundle\\nsis\\*.exe）。\n' +
                '  上面的输出里应有 makensis 的报错；若是 "mmapping file ... is out of range"，\n' +
                '  说明打包数据超过了 NSIS 的 2GB 上限。'
        );
    }

    const size = fileSize(installer);
    const payload = payloadBytes();
    // A stub is what a failed makensis leaves behind: it exits 0 often enough, and
    // a 100 KB "installer" would be copied out as if it were real.
    if (size < payload * 0.1) {
        die(
            `产物只有 ${humanSize(size)}，远小于 ${humanSize(payload)} 的载荷 —— 多半是 makensis 失败了。\n` +
                `  文件: ${installer}`
        );
    }

    ensureDir(distDir);
    const artifact = path.join(distDir, path.basename(installer));
    copyFile(installer, artifact);
    log.success(`安装包: ${artifact}  (${humanSize(size)})`);

    log.blank();
    log.plain('双击即可安装 —— 不需要证书、不需要 PowerShell、不经过商店:');
    log.plain('  安装位置  %LOCALAPPDATA%\\MonoSub（当前用户，无 UAC 提权）');
    log.plain('  附带      开始菜单快捷方式 + 设置 → 应用 里的卸载项');
    log.plain('  卸载      设置 → 应用 → MonoSub → 卸载');
    log.plain('  卸载器     安装目录下的 uninstall.exe');
    log.blank();
    log.plain('安装包未签名，首次运行 Windows SmartScreen 会拦一次：');
    log.plain('  「更多信息」→「仍要运行」。自用安装属预期行为。');
    log.blank();
}

// ---------------------------------------------------------------------------
// Steps
// ---------------------------------------------------------------------------

/**
 * Drop the previous .exe installer from dist\ and Tauri's NSIS cache.
 *
 * Both, not just dist\: the bundler reuses
 * `target\release\bundle\nsis\` and a stale file there would be collected by
 * `findInstaller()` even when this run produced nothing.
 */
function removeOldInstallers() {
    ensureDir(distDir);
    for (const entry of fs.readdirSync(distDir)) {
        if (entry.endsWith('.exe')) remove(path.join(distDir, entry));
    }
    for (const dir of installerDirs()) {
        remove(dir);
    }
}

function buildCli() {
    const source = path.join(srcTauriDir, 'src', 'bin', 'monosub-cli.rs');
    const staged = path.join(binDir, cliName);

    if (!isFile(source)) {
        // Not fatal: the app itself does not need the CLI to run, and a build must
        // not stop because a secondary binary is missing. Drop a stale copy so an
        // outdated one cannot ride into the installer.
        log.warn(`未找到 ${source}，跳过 CLI：本次安装包不含 monosub-cli`);
        remove(staged);
        return;
    }

    run('cargo', ['build', '--release', '--bin', 'monosub-cli', '--manifest-path', path.join(srcTauriDir, 'Cargo.toml')]);

    const built = path.join(srcTauriDir, 'target', 'release', 'monosub-cli.exe');
    if (!isFile(built)) die(`编译后没有 ${built}`);
    // Bare name on purpose — `resources\bin\monosub-cli.exe` is what a user types
    // and what the resolver looks for. See cliName in lib/platform.mjs.
    copyFile(built, staged);
    log.ok(`CLI 已就位: ${staged}`);
}

/**
 * Print what is about to be packed, so the 2 GB ceiling is a number the user can
 * see before a twenty-minute compression run rather than a mysterious makensis
 * error afterwards.
 */
function reportPayload() {
    const bytes = payloadBytes();
    log.plain(`  打包载荷约 ${humanSize(bytes)}（未压缩）；NSIS 的压缩后上限约 2 GB。`);
    if (bytes > 1.8 * 1024 ** 3) {
        log.warn('载荷接近 NSIS 的 2GB 数据段上限。同载荷实测 LZMA 压到约 1.31 GB，通常没问题；');
        log.warn('若 makensis 报 "mmapping file ... is out of range"，就是这个上限到了。');
    }
}

/**
 * Say out loud that `dist\` is about to lose whatever else is in it.
 *
 * dist/ is both Vite's outDir (emptyOutDir defaults to true) and the place release
 * artifacts are collected into, so `tauri build` — which runs `bun run build` —
 * deletes a previously built .msix before anything here can copy it aside. Not
 * worth archiving around (that would mean guessing which artifacts matter); worth
 * one warning line so nobody discovers it after the fact.
 */
function warnAboutViteWipe() {
    const atRisk = fs.readdirSync(distDir).filter((name) => /\.(msix|msi|dmg)$/i.test(name));
    if (atRisk.length === 0) return;
    log.warn('dist\\ 同时是 Vite 的输出目录 —— 接下来的前端构建会清空它：');
    log.warn(`  ${atRisk.join(', ')} 会被删掉（要保留就先发布或挪走）。`);
}

/**
 * Say out loud when the forked NSIS template is out of step with the CLI.
 *
 * `bundle.windows.nsis.template` points at a copy of Tauri's template, patched
 * so an upgrade cannot dead-end on the previous version's uninstaller (see
 * scripts/lib/nsis-template.mjs). The bundler fills `{{...}}` placeholders into
 * whatever template it is given, so a copy generated for an older CLI can
 * reference variables the installed one no longer substitutes — and a plain
 * copy of the stock template silently drops the upgrade fix. Neither breaks the
 * build loudly, so it is checked here, every run, before the expensive steps.
 */
function checkNsisTemplate() {
    const pinned = checkedInVersion();
    if (!pinned) {
        log.warn('src-tauri\\nsis\\installer.nsi 缺少 fork 头 —— 它可能被换成了官方模板。');
        log.warn('  升级流程的补丁会随之消失；运行 bun run nsis:sync 重新生成。');
        return;
    }
    const installed = installedTauriCliVersion();
    if (pinned !== installed) {
        log.warn(`NSIS 模板来自 tauri-cli v${pinned}，当前安装的是 v${installed}。`);
        log.warn('  先跑 bun run nsis:sync，否则模板里的 {{...}} 可能与 CLI 对不上。');
    }
}

/**
 * Say out loud when this checkout cannot install its own output.
 *
 * A process inherits the mandatory integrity level of the file it is started
 * from. When the project tree carries a Low label, the installer built here runs
 * at Low too, and Low cannot write %LOCALAPPDATA%, %APPDATA% or HKCU — so the
 * wizard stops at "Output folder: ..." having created nothing, and `/S` exits 0
 * in milliseconds without writing anything. Neither failure names the cause.
 *
 * Measured on this machine: `icacls <project>` showed
 * `Mandatory Label\Low Mandatory Level`, the exe under it ran at Low while the
 * same bytes in %TEMP% ran at Medium, and an A/B of one probe installer went
 * FAILED/FAILED -> ok/ok purely by moving the file.
 *
 * A warning rather than a failure: the build itself is fine and the artifact is
 * correct, it is only this shell that cannot launch it. `install.mjs` and
 * `check-integrity.mjs --fix` are the two ways out, both printed here.
 */
function checkIntegrityLevel() {
    const label = integrityLabel(projectRoot);
    if (label === null || label === 'Medium' || label === 'High' || label === 'System') return;
    log.warn(`本项目目录的完整性级别是 ${label} —— 从这里双击/静默运行安装包都会“成功”但什么都不装。`);
    log.warn('  从该目录启动的进程继承这个级别，而 Low 不能写 %LOCALAPPDATA%、%APPDATA%、HKCU。');
    log.warn('  修复: node scripts/release/check-integrity.mjs --fix');
    log.warn('  或直接跑 bun run release:install（会把安装包放到干净路径再运行）。');
}

/**
 * The mandatory integrity label of `dir` (`Low`/`Medium`/…), or null when it
 * carries none. `icacls` prints nothing for an unlabelled directory.
 */
function integrityLabel(dir) {
    const text = tryCapture('icacls', [dir]);
    if (!text) return null;
    const match = text.match(/Mandatory Label\\(\w+) Mandatory Level/);
    return match ? match[1] : null;
}

/** Everything the installer carries: the app binary plus the resources tree. */
function payloadBytes() {
    const exe = path.join(srcTauriDir, 'target', 'release', 'monosub.exe');
    let total = isFile(exe) ? fileSize(exe) : 0;
    if (exists(resourcesDir)) {
        for (const rel of listFiles(resourcesDir)) {
            total += fileSize(path.join(resourcesDir, ...rel.split('/')));
        }
    }
    return total;
}

/** `bundle\nsis` under the plain release dir and under any `--target` dir. */
function installerDirs() {
    const targetRoot = path.join(srcTauriDir, 'target');
    const dirs = [path.join(targetRoot, 'release', 'bundle', 'nsis')];
    if (exists(targetRoot)) {
        for (const entry of fs.readdirSync(targetRoot, { withFileTypes: true })) {
            if (!entry.isDirectory()) continue;
            dirs.push(path.join(targetRoot, entry.name, 'release', 'bundle', 'nsis'));
        }
    }
    return dirs.filter((dir) => exists(dir));
}

/** The freshest `*.exe` the bundler left behind, or null. */
function findInstaller() {
    let best = null;
    let bestTime = 0;
    for (const dir of installerDirs()) {
        for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
            if (!entry.isFile() || !entry.name.endsWith('.exe')) continue;
            const full = path.join(dir, entry.name);
            const time = fs.statSync(full).mtimeMs;
            if (time > bestTime) {
                best = full;
                bestTime = time;
            }
        }
    }
    return best;
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function runNode(script, ...rest) {
    return run(process.execPath, [script, ...rest], { cwd: projectRoot });
}

function usage() {
    console.log(`Usage: node scripts/release/installer.mjs [--skip-resources]

  仅 Windows。产出 dist\\<productName>_<version>_x64-setup.exe —— 双击安装到当前用户，
  不签名、不经商店、不需要证书（区别于 MSIX）。

  (无参数)          拉运行时 → 编译 → 打包 NSIS → 收集到 dist\\
  --skip-resources 复用已有的 resources\\（资源已就绪时可省几分钟）
  -h, --help       显示这段说明

不打版本号、不联网发布。商店包用 bun run release:package，发布用 bun run publish:msix。`);
}
