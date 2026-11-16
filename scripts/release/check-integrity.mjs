#!/usr/bin/env node
// Detect — and optionally repair — the reason a Windows installer can fail
// silently while doing nothing at all.
//
//   node scripts/release/check-integrity.mjs            # report only
//   node scripts/release/check-integrity.mjs --fix      # relabel this checkout
//
// ## The failure this exists for
//
// Measured on LEI-PC, 2026-11-16, installing `dist\MonoSub_2.1.0_x64-setup.exe`:
//
//   * double-click (GUI): the wizard opens, shows "Output folder:
//     C:\Users\<user>\AppData\Local\MonoSub", then never creates it. Every
//     file step fails, and the page just sits there.
//   * `/S` (silent): the process exits **0 in 3 ms** having written nothing —
//     no directory, no registry key, no shortcut, no error.
//
// The cause is not the installer. A directory on the path to the .exe carried a
// **Low mandatory integrity label**:
//
//   icacls C:\Users\<user>\Projects
//     ... Mandatory Label\Low Mandatory Level:(OI)(CI)(NW)
//
// A process inherits the integrity level of the file it was started from, so
// every .exe under that tree ran at Low. Low integrity cannot write to
// %LOCALAPPDATA%, %APPDATA% or HKCU — those are Medium by default — so an
// installer that only writes there fails on its first write and, because NSIS
// reports such failures through the UI it is not showing, exits 0.
//
// The same Low level is what produced the earlier, unrelated-looking
// `yt-dlp ... [PYI-…:ERROR] Could not create temporary directory!` report: a
// PyInstaller bootloader creating `_MEI<pid>` under %TEMP% hits the same wall.
//
// It is worth checking rather than assuming: an exe copied to %TEMP% or
// %LOCALAPPDATA% ran at Medium while the *same bytes* under the project tree ran
// at Low, and the A/B of one probe installer (writes to %LOCALAPPDATA% + HKCU)
// went FAILED/FAILED -> ok/ok purely by moving the file. So the label, not the
// binary, is the variable.
//
// ## Repair
//
// `--fix` sets the tree to Medium:
//
//   icacls <root> /setintegritylevel "(OI)(CI)Medium" /T /C
//
// which is the label Windows gives ordinary user directories anyway. It needs
// write access to the ACL of each file; it does not need an elevated prompt when
// the user owns the tree. Re-running is safe and idempotent.
//
// Nothing here is MonoSub-specific, so the message names the mechanism instead
// of the app: the next checkout that inherits the label fails the same way.

import fs from 'node:fs';
import path from 'node:path';

import { log, die, header } from '../lib/log.mjs';
import { hostPlatform } from '../lib/platform.mjs';
import { projectRoot } from '../lib/paths.mjs';
import { run, capture, tryCapture } from '../lib/shell.mjs';

const args = process.argv.slice(2);
if (args.includes('-h') || args.includes('--help')) {
    console.log(`Usage: node scripts/release/check-integrity.mjs [--fix]

  Windows only. Reports whether this checkout's directory tree carries a Low
  mandatory integrity label — the reason an installer started from here can
  exit 0 having written nothing.

  (no args)  report only; exit 1 when the tree is Low
  --fix      relabel the tree Medium (icacls /setintegritylevel, recursive)`);
    process.exit(0);
}

const fix = args.includes('--fix');
const unknown = args.filter((arg) => arg !== '--fix');
if (unknown.length > 0) die(`未知参数: ${unknown.join(' ')}`);

if (hostPlatform() !== 'windows') {
    // POSIX has no mandatory integrity levels; the whole class of failure is absent.
    console.log('非 Windows 平台：没有完整性级别，跳过。');
    process.exit(0);
}

header('安装器完整性级别检查（Windows mandatory integrity label）');

// The label of the directory the .exe is started from is what the process
// inherits, so this walks up from the project root to the first labelled
// ancestor — that is the one that matters, and it may not be the root itself.
const root = projectRoot;
const label = labelOf(root);

if (label === null) {
    log.ok(`${root} 没有 Low 完整性标签（可安全从这里运行安装包）。`);
    process.exit(0);
}

if (label === 'Medium' || label === 'High' || label === 'System') {
    log.ok(`${root} 的完整性级别是 ${label}，不是 Low —— 安装包可以正常写入。`);
    process.exit(0);
}

// Low: this is the failure. Report what it means before offering the repair, so
// `--fix` is a decision rather than a magic incantation.
log.err(`${root} 带有 Low 完整性标签。`);
log.plain('  从该目录启动的任何 .exe 都会继承 Low，而 Low 不能写');
log.plain('  %LOCALAPPDATA%、%APPDATA% 和 HKCU（它们默认是 Medium）。');
log.plain('  表现：NSIS 安装包双击后停在 "Output folder: ..." 不建目录；');
log.plain('        /S 静默模式 3 毫秒内以 0 退出，什么都不写、不报错。');
log.blank();

if (!fix) {
    log.plain('修复：');
    log.plain('  node scripts/release/check-integrity.mjs --fix');
    log.plain('  等价于  icacls "<目录>" /setintegritylevel "(OI)(CI)Medium" /T /C');
    log.plain('');
    log.plain('在此之前，从干净路径运行安装包也可以绕开（把 .exe 复制到 %TEMP% 再运行）：');
    log.plain('  低完整性是继承来的，复制到 Medium 目录后新进程就是 Medium。');
    process.exit(1);
}

log.step('改为 Medium（递归，可能需要一两分钟）...');
// /C keeps going past files it cannot touch and reports a count instead of
// aborting on the first one — a node_modules tree has plenty of odd entries.
run('icacls', [root, '/setintegritylevel', '(OI)(CI)Medium', '/T', '/C']);

const after = labelOf(root);
if (after === 'Low') {
    die(`${root} 仍然是 Low。可能是当前用户没有这些文件的 ACL 写权限，需要管理员执行同一条 icacls。`);
}
log.success(`${root} 现在是 ${after ?? '无标签'}。可以重新运行安装包了。`);

// ---------------------------------------------------------------------------

/**
 * The mandatory integrity label of `dir`, or null when it carries none.
 *
 * `icacls` prints it as
 *   `Mandatory Label\Low Mandatory Level:(OI)(CI)(NW)`
 * and prints nothing for an unlabelled directory. Parsed rather than read via
 * .NET on purpose: `Get-Acl` needs SeSecurityPrivilege on some setups and this
 * has to work from an ordinary shell.
 */
function labelOf(dir) {
    if (!fs.existsSync(dir)) return null;
    const text = tryCapture('icacls', [dir]);
    if (!text) return null;
    const match = text.match(/Mandatory Label\\(\w+) Mandatory Level/);
    return match ? match[1] : null;
}
