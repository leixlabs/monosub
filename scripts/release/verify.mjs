#!/usr/bin/env node
// Verify the artifact that is about to be published.
//
//   node scripts/release/verify.mjs <path-to-dmg-or-msix>
//
// Two branches, one purpose: a package that builds successfully can still be
// unusable, and every failure mode here is one that produced no error at build
// time.
//
//   macOS    the DMG's App must be ad-hoc signed and its bundled engines must
//            actually start. "Signed but the engine cannot launch" is the classic
//            silent failure: ad-hoc + hardened runtime makes dyld reject the
//            dylibs llama-server loads from @loader_path, so the app opens and
//            translation simply does nothing.
//   Windows  the MSIX must carry both engines in their own directories (they each
//            ship a ggml-cuda.dll built against a different CUDA toolkit), the
//            manifest identity must be right, and the CLI's execution alias must
//            be declared — a package with the file but no alias passes packing
//            and only fails when a user types `monosub-cli`.
//
// Exits non-zero on the first failure. The packaging script calls this and aborts.

import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';

import { log, die } from '../lib/log.mjs';
import { hostPlatform, targetTriple, cliName } from '../lib/platform.mjs';
import { makeScratchDir, remove } from '../lib/fs.mjs';
import { run, tryCapture, tryCaptureMerged } from '../lib/shell.mjs';
import { listZip, readZipEntry } from '../lib/archive.mjs';
import * as xml from '../lib/xml.mjs';

const artifact = process.argv[2];
const platform = hostPlatform();

if (process.argv.includes('--help') || process.argv.includes('-h')) {
    log.plain(`Usage: node scripts/release/verify.mjs <path-to-dmg-or-msix>

  <path>      要校验的产物：*.dmg 或 *.msix
  -h, --help  显示这段说明

任一检查失败就以非零码退出。`);
    process.exit(0);
}

if (!artifact || !fs.existsSync(artifact)) {
    die(`artifact not found: ${artifact ?? '<none given>'}`);
}

main().catch((error) => {
    log.err(error.message);
    process.exit(1);
});

async function main() {
    if (platform === 'macos' && artifact.endsWith('.dmg')) {
        verifyDmg(artifact);
    } else if (platform === 'windows' && artifact.endsWith('.msix')) {
        await verifyMsix(artifact);
    } else {
        die(`cannot verify ${artifact} on ${platform} (expected a .dmg on macOS, a .msix on Windows)`);
    }
}

// ---------------------------------------------------------------------------
// macOS — DMG
// ---------------------------------------------------------------------------

/**
 * Verify the DMG that will be published, not the .app on disk: the .app may be
 * stale, and it is the DMG's copy that a user actually opens.
 *
 * Signing is configured in tauri.conf.json
 * (bundle.macOS.signingIdentity = "-", hardenedRuntime = false) and performed by
 * Tauri before it builds the DMG. This script only checks — re-signing here would
 * sign a copy that is not in the DMG, which is exactly what the old script did.
 */
function verifyDmg(dmgPath) {
    // No Developer ID on this machine. A leftover signing identity in the
    // environment would override the ad-hoc configuration and produce a package
    // that looks signed and will not open for anyone else.
    const identity = process.env.APPLE_SIGNING_IDENTITY;
    if (identity && identity !== '-') {
        die(`APPLE_SIGNING_IDENTITY=${identity} overrides ad-hoc signing.
  There is no Apple Developer account here; distribution must be ad-hoc
  (signingIdentity = "-"). Unset APPLE_SIGNING_IDENTITY before building.`);
    }

    const mountPoint = makeScratchDir('monosub-dmg-verify-');
    let detached = false;
    const detach = () => {
        if (detached) return;
        tryCapture('hdiutil', ['detach', mountPoint, '-quiet']);
        tryCapture('hdiutil', ['detach', mountPoint, '-force', '-quiet']);
        detached = true;
    };

    try {
        log.step('挂载 DMG 以校验产物...');
        run('hdiutil', ['attach', dmgPath, '-nobrowse', '-readonly', '-mountpoint', mountPoint]);

        const appPath = findApp(mountPoint);
        if (!appPath) die('DMG 内没有找到 .app');
        log.step(`  已找到: ${path.basename(appPath)}`);

        // 1. The signature itself: present, intact, and ad-hoc — not a developer
        //    certificate that happens to exist on this machine.
        log.step('1. 校验 App 签名...');
        run('codesign', ['--verify', '--deep', '--strict', appPath], {
            stdio: ['ignore', 'ignore', 'pipe'],
        });

        // `codesign -dv` writes to stderr, so both streams are merged here.
        const signOutput = tryCaptureMerged('codesign', ['-dv', appPath]) ?? '';
        if (!/Signature=adhoc/.test(signOutput)) {
            die('App 不是 ad-hoc 签名。若看到 Authority=Apple Development / Developer ID，说明构建复用了本机证书——这不是可分发签名。');
        }
        if (/^Authority=(Apple Development|Developer ID)/m.test(signOutput)) {
            die('App 带了本机开发者证书签名，必须改为 ad-hoc。');
        }
        // hardenedRuntime + ad-hoc = no Team ID, so dyld's library validation
        // refuses llama-server's @loader_path dylibs. Measured: "mapping process
        // and mapped file (non-platform) have different Team IDs".
        if (/flags=.*runtime/.test(signOutput)) {
            die('App 启用了 hardened runtime。ad-hoc + runtime 会让 llama-server 加载不了同目录的 dylib（实测）。请确认 tauri.conf.json 里 hardenedRuntime=false。');
        }
        log.success('  App 为 ad-hoc 签名，未启用 hardened runtime');

        // 2. Every bundled binary must be ad-hoc too.
        const resourceBinDir = path.join(appPath, 'Contents', 'Resources', 'resources', 'bin');
        if (!fs.existsSync(resourceBinDir)) die(`DMG 内缺少内嵌二进制目录: ${resourceBinDir}`);

        log.step('2. 校验内嵌二进制签名...');
        let binCount = 0;
        for (const entry of fs.readdirSync(resourceBinDir)) {
            const full = path.join(resourceBinDir, entry);
            if (!fs.statSync(full).isFile()) continue;
            binCount += 1;
            const detail = tryCaptureMerged('codesign', ['-dv', full]);
            if (!detail || !/Signature=adhoc/.test(detail)) {
                die(`内嵌二进制不是 ad-hoc 签名: ${entry}`);
            }
        }
        log.success(`  ${binCount} 个内嵌二进制均为 ad-hoc 签名`);

        // 3. Smoke test: run each engine from inside the mounted DMG. This is the
        //    step that catches the hardened-runtime failure described above.
        log.step('3. 冒烟测试内嵌引擎（从 DMG 内的副本运行）...');
        runQuietOk(`${resourceBinDir}/ffprobe-${targetTriple()}`, ['-version'], 'ffprobe');
        log.success('  ffprobe 正常');

        runQuietOk(`${resourceBinDir}/audiocpp_cli-${targetTriple()}`, ['--version'], 'audiocpp_cli');
        log.success('  audiocpp_cli 正常');

        const llamaOut = tryCaptureMerged(`${resourceBinDir}/llama-server-${targetTriple()}`, ['--version']);
        if (llamaOut === null || !/version/i.test(llamaOut)) {
            die('llama-server 无法启动——通常是 hardened runtime 破坏了 @loader_path 的 dylib 加载');
        }
        log.success('  llama-server 正常（dylib 加载通过）');

        // Bare `monosub-cli`, no target triple: it is the name a user types, so
        // the packaged file must answer exactly that lookup (see `cliName`).
        const cliBin = fs
            .readdirSync(resourceBinDir)
            .find((entry) => entry === cliName);
        if (cliBin) {
            runQuietOk(path.join(resourceBinDir, cliBin), ['--help'], 'monosub-cli');
            log.success('  monosub-cli 正常');
        } else {
            log.warn('  未找到 monosub-cli，跳过（本次构建不含 CLI）');
        }

        detach();

        // 4. Notarisation: there is no Developer account, so there is nothing that
        //    can be notarised. Skipping is expected, not a failure.
        log.step('4. 公证检查...');
        if (process.env.APPLE_ID && process.env.APPLE_PASSWORD && process.env.APPLE_TEAM_ID) {
            log.warn('检测到 Apple Notary 凭据，但产物是 ad-hoc 签名，公证会失败，已跳过。');
            log.warn('ad-hoc 签名的 App 无法公证；用户首次打开需在 系统设置 → 隐私与安全性 放行。');
        } else {
            log.step('  无 Apple Notary 凭据，跳过公证（ad-hoc 分发预期如此）');
        }

        log.success(`产物校验通过: ${dmgPath}`);
    } finally {
        detach();
        remove(mountPoint);
    }
}

// ---------------------------------------------------------------------------
// Windows — MSIX
// ---------------------------------------------------------------------------

/**
 * The identity Partner Center accepts for this product, quoted verbatim from its
 * own rejections:
 *
 *   Invalid package identity name: monosub (expected: leixlabs.monosub)
 *   Invalid package publisher name: CN=leix
 *       (expected: CN=26B9D6B3-981A-4C13-BC1E-234DFA00861E)
 *
 * Kept here as a *contract*, deliberately duplicated from package.mjs: if the
 * packaging script and this table ever disagree, that disagreement is the bug and
 * the build must stop rather than ship whichever value happened to win.
 */
const STORE_IDENTITY = {
    name: 'leixlabs.monosub',
    publisher: 'CN=26B9D6B3-981A-4C13-BC1E-234DFA00861E',
    publisherDisplayName: 'leixlabs',
    // What Partner Center reports for that identity; recomputed below.
    packageFamilyName: 'leixlabs.monosub_ay3jbxdght5hg',
};

/**
 * Package family name = `<Identity Name>_<13-char hash>`, where the hash is
 * SHA-256 over the publisher string encoded as UTF-16LE, first 8 bytes, written
 * in Microsoft's 32-character base32 alphabet, left-aligned into 13 characters
 * (the last character therefore carries 4 bits shifted up by one).
 *
 * Recomputed rather than trusted because Windows derives it at install time: a
 * publisher typo produces a *different* family name, which is what the Store
 * reports back as `Invalid package family name`.
 */
function packageFamilyName(name, publisher) {
    const ALPHABET = '0123456789abcdefghjkmnpqrstvwxyz';
    const digest = crypto.createHash('sha256').update(Buffer.from(publisher, 'utf16le')).digest();
    let bits = '';
    for (const byte of digest.subarray(0, 8)) bits += byte.toString(2).padStart(8, '0');
    bits += '0';
    let hash = '';
    for (let i = 0; i < bits.length; i += 5) hash += ALPHABET[parseInt(bits.slice(i, i + 5), 2)];
    return `${name}_${hash}`;
}

async function verifyMsix(msixPath) {
    const triple = targetTriple();
    const failures = [];
    const addFailure = (message) => failures.push(message);

    const baseName = path.basename(msixPath);
    if (!/_x64(\.signed)?\.msix$/.test(baseName)) {
        addFailure(`File name does not end in _x64.msix: ${baseName}`);
    } else {
        log.ok(`File name declares x64: ${baseName}`);
    }

    const entries = new Set((await listZip(msixPath)).map((entry) => entry.name));
    const requireEntry = (entry, description) => {
        if (entries.has(entry)) {
            log.ok(`${description}: ${entry}`);
        } else {
            addFailure(`${description} missing from the package: ${entry}`);
        }
    };

    requireEntry('AppxManifest.xml', 'Manifest');
    requireEntry('monosub.exe', 'Entry point');
    // Both engines live in a directory of their own: each ships a ggml-cuda.dll
    // built against a different CUDA toolkit, so they cannot share a flat bin\.
    requireEntry(`resources/bin/audiocpp_cli/audiocpp_cli-${triple}.exe`, 'audio.cpp CLI');
    requireEntry(`resources/bin/llama-server/llama-server-${triple}.exe`, 'llama.cpp server');
    requireEntry(`resources/bin/ffmpeg-${triple}.exe`, 'FFmpeg');
    requireEntry(`resources/bin/ffprobe-${triple}.exe`, 'FFprobe');
    requireEntry(`resources/bin/yt-dlp-${triple}.exe`, 'yt-dlp');
    requireEntry(`resources/bin/${cliName}`, 'Command-line tool');
    requireEntry('resources/models.json', 'Model catalogue');
    requireEntry('resources/models/silero_vad/silero_vad_16k.safetensors', 'Silero VAD model');

    // A missing DLL only shows up as a crash at run time, so require at least one
    // of each family the two engines need — per engine, because a ggml-cuda.dll in
    // the wrong directory is as fatal as none at all and invisible in a file count.
    const dllCount = [...entries].filter((name) => /^resources\/bin\/.*\.dll$/.test(name)).length;
    if (dllCount >= 10) {
        log.ok(`Runtime DLLs present (${dllCount})`);
    } else {
        addFailure(`Only ${dllCount} DLL(s) in the package; the engines cannot start`);
    }

    // CUDA **13** specifically: a stale `cudart64_12.dll` here would mean a
    // previous 12.4 experiment survived a rebuild, and the package would ship
    // the build that has no Blackwell kernels.
    // The engines live in their own directories. A flat copy of either
    // executable is a leftover from the pre-subdirectory layout — and a stale
    // CPU-only `audiocpp_cli-<triple>.exe` in `bin\` is exactly what once made
    // the app run an engine with no CUDA backend.
    const flatEngines = [...entries].filter((name) =>
        /^resources\/bin\/(audiocpp_cli|llama-server)-[^/]*\.exe$/.test(name),
    );
    if (flatEngines.length > 0) {
        addFailure(`Flat engine executable(s) in resources/bin: ${flatEngines.join(', ')} — the engines must only live in their own subdirectory`);
    } else {
        log.ok('No flat engine executables in resources/bin');
    }

    requireEntry('resources/bin/audiocpp_cli/ggml-cuda.dll', 'audio.cpp CUDA backend');
    requireEntry('resources/bin/audiocpp_cli/cudart64_13.dll', 'audio.cpp CUDA runtime');
    requireEntry('resources/bin/audiocpp_cli/cublas64_13.dll', 'audio.cpp cuBLAS');
    // cuFFT carries its own major (12 in the 13.x toolkits), so it is matched by
    // family rather than by a number that tracks something else.
    if ([...entries].some((name) => /^resources\/bin\/audiocpp_cli\/cufft64_\d+\.dll$/.test(name))) {
        log.ok('audio.cpp cuFFT present');
    } else {
        addFailure('resources/bin/audiocpp_cli/cufft64_*.dll missing; audio.cpp cannot inverse-STFT');
    }
    requireEntry('resources/bin/llama-server/ggml-cuda.dll', 'llama.cpp CUDA backend');
    requireEntry('resources/bin/llama-server/cudart64_13.dll', 'llama.cpp CUDA runtime');
    requireEntry('resources/bin/llama-server/cublas64_13.dll', 'llama.cpp cuBLAS');
    requireEntry('resources/bin/llama-server/llama-server-impl.dll', 'llama.cpp server implementation');
    requireEntry('resources/bin/vcruntime140.dll', 'MSVC runtime');

    // --- Manifest ---------------------------------------------------------
    const manifestText = await readZipEntry(msixPath, 'AppxManifest.xml');
    if (!manifestText) {
        addFailure('AppxManifest.xml is unreadable; every manifest check below is skipped');
    } else {
        // WebView2 must NOT be declared as a PackageDependency: it is an Evergreen
        // runtime, not an MSIX framework, and declaring it makes Add-AppxPackage
        // fail with 0x80073CF3 even on machines that have WebView2.
        if (/Microsoft\.Win32WebView2/.test(manifestText)) {
            addFailure('Manifest declares Microsoft.Win32WebView2 as a framework dependency; WebView2 is an Evergreen runtime');
        } else {
            log.ok('No WebView2 framework dependency (correct: it is an Evergreen runtime)');
        }

        const root = xml.root(manifestText, 'Package');

        const identity = xml.findChild(manifestText, root, 'Identity');
        if (!identity) {
            addFailure('Manifest has no <Identity>');
        } else {
            const version = xml.getAttr(identity, 'Version');
            if (/^\d+\.\d+\.\d+\.\d+$/.test(version ?? '')) {
                log.ok(`Manifest version: ${version}`);
            } else {
                addFailure(`Manifest Version is not four-part: '${version}'`);
            }
            const arch = xml.getAttr(identity, 'ProcessorArchitecture');
            if (arch === 'x64') {
                log.ok('Processor architecture: x64');
            } else {
                addFailure(`Identity declares ProcessorArchitecture '${arch}'; expected x64`);
            }

            // The identity the Store validates. `MONOSUB_MSIX_*` overrides exist
            // so an experimental build can still be verified, but then only
            // self-consistency is checked — the Store PFN is not asserted.
            const overrideName = process.env.MONOSUB_MSIX_NAME;
            const overridePublisher = process.env.MONOSUB_MSIX_PUBLISHER;
            const expectedName = overrideName || STORE_IDENTITY.name;
            const expectedPublisher = overridePublisher || STORE_IDENTITY.publisher;

            const name = xml.getAttr(identity, 'Name');
            if (name === expectedName) {
                log.ok(`Identity name: ${name}`);
            } else {
                addFailure(`Identity name is '${name}'; the Store expects '${expectedName}'`);
            }

            const publisher = xml.getAttr(identity, 'Publisher');
            if (publisher === expectedPublisher) {
                log.ok(`Identity publisher: ${publisher}`);
            } else {
                addFailure(`Identity Publisher is '${publisher}'; the Store expects '${expectedPublisher}'`);
            }

            const pfn = packageFamilyName(name ?? '', publisher ?? '');
            const expectedPfn = overrideName || overridePublisher
                ? packageFamilyName(expectedName, expectedPublisher)
                : STORE_IDENTITY.packageFamilyName;
            if (pfn === expectedPfn) {
                log.ok(`Package family name: ${pfn}`);
            } else {
                addFailure(`Package family name would be '${pfn}'; the Store expects '${expectedPfn}'`);
            }
        }

        const applications = xml.findChild(manifestText, root, 'Applications');
        const application = applications ? xml.findChild(manifestText, applications, 'Application') : null;
        if (!application) {
            addFailure('Manifest declares no <Application>');
        } else {
            if (xml.getAttr(application, 'Executable') === 'monosub.exe') {
                log.ok('Entry point pinned to monosub.exe');
            } else {
                addFailure('Manifest entry point is not monosub.exe');
            }

            // Case-sensitive on purpose: 'monosub' and 'MonoSub' compare equal
            // case-insensitively, and the lowercase form is exactly the defect this
            // check exists to catch.
            const visual = xml.findChild(manifestText, application, 'VisualElements');
            const visualName = visual ? xml.getAttr(visual, 'DisplayName') : null;
            if (visualName === 'MonoSub') {
                log.ok('Start-menu DisplayName: MonoSub');
            } else {
                addFailure(`VisualElements DisplayName is '${visualName}'; the Start menu would show the lowercase name`);
            }
        }

        const properties = xml.findChild(manifestText, root, 'Properties');
        const propertiesName = properties ? xml.childText(manifestText, properties, 'DisplayName') : '';
        if (propertiesName === 'MonoSub') {
            log.ok('Properties DisplayName: MonoSub');
        } else {
            addFailure(`Properties DisplayName is '${propertiesName}'; expected 'MonoSub'`);
        }

        // Must equal the Partner Center publisher display name character for
        // character; Store upload rejects the package otherwise and the message
        // only names the two values.
        const publisherDisplayName = properties ? xml.childText(manifestText, properties, 'PublisherDisplayName') : '';
        const expectedPublisherDisplayName = process.env.MONOSUB_MSIX_PUBLISHER_DISPLAY_NAME || STORE_IDENTITY.publisherDisplayName;
        if (publisherDisplayName === expectedPublisherDisplayName) {
            log.ok(`PublisherDisplayName: ${publisherDisplayName}`);
        } else {
            addFailure(`PublisherDisplayName is '${publisherDisplayName}'; expected '${expectedPublisherDisplayName}' to match Partner Center`);
        }

        if (/monosub-cli\.exe/.test(manifestText)) {
            log.ok('Command-line alias declared: monosub-cli.exe');
        } else {
            addFailure('No monosub-cli.exe execution alias in the manifest; the CLI is unreachable from a terminal');
        }
    }

    if (failures.length > 0) {
        for (const failure of failures) log.err(failure);
        die(`${failures.length} check(s) failed: ${baseName} is not publishable`);
    }

    log.success(`产物校验通过: ${msixPath}`);
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function findApp(mountPoint) {
    for (const entry of fs.readdirSync(mountPoint)) {
        if (entry.endsWith('.app')) return path.join(mountPoint, entry);
    }
    return null;
}

/** Run a binary quietly; throw with `label` when it cannot start. */
function runQuietOk(binary, args, label) {
    const result = tryCapture(binary, args, { stdio: ['ignore', 'pipe', 'pipe'] });
    if (result === null) die(`${label} 无法启动`);
    return result;
}
