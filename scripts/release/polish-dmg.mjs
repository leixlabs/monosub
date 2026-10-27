#!/usr/bin/env node
// Give the DMG window the geometry tauri.conf.json asks for.
//
//   node scripts/release/polish-dmg.mjs <path-to-dmg>
//
// Why this step exists
// --------------------
// Tauri's official DMG pipeline is still the one that builds this file — the
// background, window size and icon positions all come from tauri.conf.json, and
// create-dmg's AppleScript applies them. What it cannot do is leave them alone
// afterwards: whenever `--background` is passed, the script ends its positioning
// block with
//
//     set position of every item to {theBottomRightX + 100, 100}
//
// which parks `.VolumeIcon.icns` and `.background` exactly 100 pt to the right of
// the window. Measured on macOS 26, Finder reacts in one of two ways, and both
// are visible defects:
//
//   * it grows the window to fit them (908x595 instead of 660x400), so the
//     background image only covers the top-left corner and the volume icon ends
//     up inside the window — the "duplicate icon" report;
//   * or it keeps 660x400 and shows scrollbars, because the icon view's content
//     is wider than the view.
//
// Neither can be fixed from configuration: the +100 is relative to the window's
// own right edge, so the parked items are always outside it.
//
// What this does
// --------------
// Mounts a writable copy, then rewrites three things in `.DS_Store` in place and
// hides the dot-entries at the filesystem level:
//
//   * bwsp.WindowBounds  -> windowSize / windowPosition from tauri.conf.json
//   * Iloc of the .app and Applications -> appPosition / applicationFolderPosition
//   * Iloc of the dot-entries -> back inside the window (UF_HIDDEN means Finder
//     draws nothing, but they must not extend the icon view either)
//
// The edits are same-length byte replacements, so the Bud1 container's offset
// table stays valid and no Finder round-trip is needed: the step runs unattended
// in a build, in about two seconds.

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import { log, die } from '../lib/log.mjs';
import { tauriConfPath } from '../lib/paths.mjs';
import { ensureDir, makeScratchDir, remove, copyFile } from '../lib/fs.mjs';
import { run, tryCapture } from '../lib/shell.mjs';

const DS_STORE = '.DS_Store';
/** Where the dot-entries are parked: inside the window, away from both icons. */
const PARKED = { x: 20, y: 26 };

// ---------------------------------------------------------------------------
// Bud1 (.DS_Store) reading
// ---------------------------------------------------------------------------

/** Record types that carry a blob or a scalar we may want to read. */
const DATA_TYPES = new Set(['blob', 'long', 'bool', 'ustr', 'type', 'shct']);

/**
 * Locate records by their four-character code.
 *
 * A record — as laid out in the file, despite what most write-ups claim — is:
 *
 *     u32  name length in UTF-16 code units
 *     name UTF-16BE, that many units
 *     u32  record code  ('bwsp', 'Iloc', …)
 *     u32  data type    ('blob', 'long', …)
 *     u32  data length in bytes
 *     data
 *
 * There is no per-record length field, so the name is recovered by scanning
 * backwards for a plausible length prefix whose payload decodes cleanly. That is
 * how the file is actually written, and it is why the forward walk below can be
 * code-driven: a code is only accepted once a name has been recovered for it.
 */
function findRecords(buffer, code) {
    const needle = Buffer.from(code, 'latin1');
    const records = [];
    let from = 0;

    for (;;) {
        const at = buffer.indexOf(needle, from);
        if (at < 0) return records;
        from = at + 4;

        const type = buffer.subarray(at + 4, at + 8).toString('latin1');
        if (!DATA_TYPES.has(type)) continue;

        const dataLength = buffer.readUInt32BE(at + 8);
        if (dataLength > 8192) continue;

        const name = nameBefore(buffer, at);
        if (name === null) continue;

        records.push({ name, dataOffset: at + 12, dataLength });
    }
}

function nameBefore(buffer, codeOffset) {
    for (let units = 0; units < 96; units += 1) {
        const start = codeOffset - units * 2 - 4;
        if (start < 0) return null;
        if (buffer.readUInt32BE(start) !== units) continue;
        // Names are UTF-16**BE**; Node has no BE decoder, so swap and reuse LE.
        const raw = Buffer.from(buffer.subarray(start + 4, codeOffset));
        if (raw.length !== units * 2) continue;
        const candidate = raw.swap16().toString('utf16le');
        // Reject the false positives a backwards scan otherwise invents: every
        // UTF-16BE name is also valid UTF-16LE byte soup, so the test is that the
        // decoded text has no control characters, not merely that it decodes.
        if (candidate.length === units && !/[\u0000-\u001f]/.test(candidate)) return candidate;
    }
    return null;
}

/** Read the four numbers out of `{{x, y}, {w, h}}`. */
function parseBounds(text) {
    const numbers = text.replace(/[{}]/g, ' ').split(',').flatMap((part) => part.trim().split(/\s+/));
    const values = numbers.filter(Boolean).map(Number);
    if (values.length !== 4 || values.some((value) => !Number.isFinite(value))) return null;
    return { x: values[0], y: values[1], width: values[2], height: values[3] };
}

/**
 * Render bounds in whichever spacing variant is exactly as long as the string it
 * replaces. In-place replacement is only safe while the byte count is unchanged
 * (the plist and the offset table around it are position-based), and Finder
 * accepts the comma spacing either way.
 */
function renderBounds(bounds, targetLength) {
    const variants = [
        `{{${bounds.x}, ${bounds.y}}, {${bounds.width}, ${bounds.height}}}`,
        `{{${bounds.x},${bounds.y}},{${bounds.width},${bounds.height}}}`,
        `{{${bounds.x}, ${bounds.y}},{${bounds.width},${bounds.height}}}`,
        `{{${bounds.x},${bounds.y}}, {${bounds.width}, ${bounds.height}}}`,
    ];
    return variants.find((text) => text.length === targetLength) ?? null;
}

// ---------------------------------------------------------------------------
// Patching
// ---------------------------------------------------------------------------

function patchDsStore(dsStorePath, spec) {
    const buffer = fs.readFileSync(dsStorePath);
    const changes = [];

    // --- window geometry ---------------------------------------------------
    const [bwsp] = findRecords(buffer, 'bwsp');
    if (!bwsp) die(`${DS_STORE} 里没有 bwsp 记录，无法确定窗口几何`);

    const blob = buffer.subarray(bwsp.dataOffset, bwsp.dataOffset + bwsp.dataLength);
    const stored = /\{\{-?\d+,\s*-?\d+\},\s*\{-?\d+,\s*-?\d+\}\}/.exec(blob.toString('latin1'))?.[0];
    const current = stored ? parseBounds(stored) : null;
    if (!current) die(`解析 bwsp.WindowBounds 失败: ${JSON.stringify(stored)}`);

    // The stored y is a *bottom-left* origin in global screen coordinates, so the
    // screen height has to come back out of it before a new top-left can go in.
    // Deriving it from the value create-dmg wrote keeps the window's vertical
    // placement identical to what it would have been on the build machine.
    const screenHeight = current.y + current.height + spec.windowPosition.y;
    const want = {
        x: spec.windowPosition.x,
        y: screenHeight - spec.windowPosition.y - spec.windowSize.height,
        width: spec.windowSize.width,
        height: spec.windowSize.height,
    };

    if (current.width === want.width && current.height === want.height
        && current.x === want.x && current.y === want.y) {
        changes.push(`WindowBounds 已是 ${stored}`);
    } else {
        const replacement = renderBounds(want, stored.length);
        if (!replacement) {
            die(`无法在等长前提下重写 WindowBounds（${stored.length} 字节）。`
                + '这通常意味着窗口尺寸的位数变了，请检查 tauri.conf.json 的 dmg.windowSize');
        }
        const at = blob.indexOf(Buffer.from(stored, 'latin1'));
        buffer.write(replacement, bwsp.dataOffset + at, 'latin1');
        changes.push(`WindowBounds ${stored} -> ${replacement}`);
    }

    // --- icon positions ----------------------------------------------------
    const named = new Map([
        [spec.appName, spec.appPosition],
        ['Applications', spec.applicationFolderPosition],
    ]);

    for (const record of findRecords(buffer, 'Iloc')) {
        if (record.dataLength < 8) continue;

        let target = named.get(record.name) ?? null;
        if (!target) {
            // Dot-entries: hidden from view, but their stored position still
            // widens the icon view, which is what produces the scrollbars.
            if (!record.name.startsWith('.') || record.name === '.') continue;
            target = PARKED;
        }

        const x = buffer.readUInt32BE(record.dataOffset);
        const y = buffer.readUInt32BE(record.dataOffset + 4);
        if (x === target.x && y === target.y) continue;

        buffer.writeUInt32BE(target.x, record.dataOffset);
        buffer.writeUInt32BE(target.y, record.dataOffset + 4);
        changes.push(`${record.name} (${x},${y}) -> (${target.x},${target.y})`);
    }

    fs.writeFileSync(dsStorePath, buffer);
    return changes;
}

// ---------------------------------------------------------------------------
// Pipeline
// ---------------------------------------------------------------------------

const HIDDEN_ENTRIES = ['.VolumeIcon.icns', '.background'];

function polish(dmgPath) {
    const conf = JSON.parse(fs.readFileSync(tauriConfPath, 'utf8'));
    const dmg = conf?.bundle?.macOS?.dmg ?? {};
    if (!dmg.background) {
        // Without a background create-dmg never runs the repositioning clause, so
        // the window keeps the size it was given. Nothing to repair.
        log.warn('未配置 dmg.background，跳过 DMG 窗口修补（create-dmg 不会重定位隐藏文件）');
        return;
    }

    const spec = {
        windowSize: dmg.windowSize ?? { width: 660, height: 400 },
        windowPosition: dmg.windowPosition ?? { x: 200, y: 120 },
        appPosition: dmg.appPosition ?? { x: 180, y: 170 },
        applicationFolderPosition: dmg.applicationFolderPosition ?? { x: 480, y: 170 },
        appName: `${conf.productName}.app`,
    };

    const scratch = makeScratchDir('monosub-dmg-polish-');
    const rwDmg = path.join(scratch, 'rw.dmg');
    const outDmg = path.join(scratch, 'out.dmg');
    const mountPoint = path.join(scratch, 'mnt');
    ensureDir(mountPoint);

    let attached = false;
    const detach = () => {
        if (!attached) return;
        tryCapture('hdiutil', ['detach', mountPoint, '-quiet']);
        tryCapture('hdiutil', ['detach', mountPoint, '-force', '-quiet']);
        attached = false;
    };

    try {
        log.step('解压为可写镜像...');
        run('hdiutil', ['convert', dmgPath, '-format', 'UDRW', '-o', rwDmg, '-quiet'], {
            stdio: ['ignore', 'ignore', 'ignore'],
        });

        run('hdiutil', ['attach', rwDmg, '-nobrowse', '-noautoopen', '-mountpoint', mountPoint], {
            stdio: ['ignore', 'ignore', 'ignore'],
        });
        attached = true;

        // The dot-entries never need to be *seen*: UF_HIDDEN keeps Finder from
        // drawing them even though they carry an icon position again.
        const present = HIDDEN_ENTRIES
            .map((entry) => path.join(mountPoint, entry))
            .filter((full) => fs.existsSync(full));
        if (present.length > 0) {
            run('chflags', ['hidden', ...present], { stdio: ['ignore', 'ignore', 'ignore'] });
        }

        const dsStore = path.join(mountPoint, DS_STORE);
        if (!fs.existsSync(dsStore)) die(`DMG 内没有 ${DS_STORE}，create-dmg 的 Finder 步骤可能没跑`);
        for (const change of patchDsStore(dsStore, spec)) log.plain(`  ${change}`);

        // Flush before detaching: hdiutil converts the file that is on disk.
        run('sync', [], { stdio: ['ignore', 'ignore', 'ignore'] });
        detach();

        log.step('重新压缩为只读镜像...');
        run('hdiutil', ['convert', rwDmg, '-format', 'UDZO', '-o', outDmg, '-quiet'], {
            stdio: ['ignore', 'ignore', 'ignore'],
        });
        copyFile(outDmg, dmgPath);
        log.success(`DMG 窗口几何已修正: ${path.basename(dmgPath)}`);
    } finally {
        detach();
        remove(scratch);
    }
}

// ---------------------------------------------------------------------------
// CLI
// ---------------------------------------------------------------------------

const isMain = process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url);

if (isMain) {
    const target = process.argv[2];
    if (!target || process.argv.includes('--help') || process.argv.includes('-h')) {
        log.plain(`Usage: node scripts/release/polish-dmg.mjs <path-to-dmg>

  <path>      要修补的 DMG（原地替换）
  -h, --help  显示这段说明

按 tauri.conf.json 的 dmg 配置修正安装窗口尺寸、图标位置，并把
.VolumeIcon.icns / .background 重新隐藏，避免滚动条和重复图标。`);
        process.exit(target ? 0 : 1);
    }
    if (!fs.existsSync(target)) die(`找不到 DMG: ${target}`);
    polish(target);
}

export { polish, patchDsStore, findRecords };
