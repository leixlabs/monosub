#!/usr/bin/env node
// Prepare the runtime binaries the app bundles, for whichever platform this is.
//
//   node scripts/release/prepare-resources.mjs
//
// One script, two branches: macOS and Windows need different artifacts from the
// same upstreams, and the layout differs outright on Windows. Everything that is
// genuinely shared — the Silero VAD weights, yt-dlp and its licences, the
// resource manifest — is done once at the end for both.
//
// Idempotent: re-running with matching artifacts is a no-op, so the packaging
// script can call it on every build without paying for the downloads twice.
//
// ## Windows layout, and why each engine has its own directory
//
//   bin\audiocpp_cli\audiocpp_cli-x86_64-pc-windows-msvc.exe   audio.cpp CLI (CUDA 13.3)
//   bin\audiocpp_cli\ggml*.dll, cudart64_13.dll, cublas64_13.dll,
//                    cublasLt64_13.dll, cufft64_12.dll          its CUDA runtime
//   bin\llama-server\llama-server-x86_64-pc-windows-msvc.exe   llama.cpp server (CUDA 13.4)
//   bin\llama-server\llama*.dll, ggml*.dll, mtmd.dll, libomp.dll + CUDA runtime
//   bin\ffmpeg-<triple>.exe, bin\ffprobe-<triple>.exe           static ffmpeg
//   bin\yt-dlp-<triple>.exe
//   bin\msvcp140*.dll, bin\vcruntime140*.dll                    MSVC runtime, flat
//
// Both engines are ggml-based and both ship a CUDA build, so both carry a
// `ggml-cuda.dll` — 258 MB from audio.cpp (CUDA 13.3), 149 MB from llama.cpp
// (CUDA 13.4). They are not interchangeable: different ggml ABIs, different CUDA
// toolkits, and their cuBLAS/cuFFT redistributables differ too. Flat in one
// `bin\` the second install silently overwrote the first, and whichever engine
// lost the race could not start.
//
// Subdirectories work because Windows resolves a process's imports from the
// directory its executable was loaded from first, so an `.exe` next to its own
// DLLs is enough — no PATH, manifest or SetDllDirectory involved.
//
// The macOS layout is unaffected: audio.cpp is static there and llama's dylibs
// are already flat, so nothing collides.
//
// ## Why CUDA and not CPU
//
// CPU inference is far too slow to ship: recognising a one-hour recording runs
// for many minutes where the CUDA backend finishes in a fraction of that.
// Windows therefore fetches the CUDA engine and runs `--backend cuda` only — see
// `Backend::platform_default()` in src-tauri/src/engine/cli.rs. There is no CPU
// build to download and no CPU retry when a CUDA run fails.
//
// CUDA 13.x does mean Pascal and older (GTX 10-series and below) are out: NVIDIA
// dropped those architectures in CUDA 13. It is the version that covers Blackwell
// (RTX 50-series, sm_120), which CUDA 12.x does not.

import path from 'node:path';
import fs from 'node:fs';

import { log, die, humanSize } from '../lib/log.mjs';
import { hostPlatform, targetTriple, bundledName } from '../lib/platform.mjs';
import {
    resourcesDir,
    binDir,
    sileroDir,
    licensesDir,
    manifestPath,
} from '../lib/paths.mjs';
import { ensureDir, copyFile, remove, isExecutable, listFiles, fileSize, makeScratchDir, writeTextFile, filesWithPrefix } from '../lib/fs.mjs';
import { download, assertSha256, matchesSha256 } from '../lib/net.mjs';
import { extractZip, extractTarGz } from '../lib/archive.mjs';
import { run, tryCapture } from '../lib/shell.mjs';

// --- Pinned upstream ------------------------------------------------------
// Every pin is the archive the URL resolves to today. Bump them together after
// a smoke test of the packaged app, never one at a time.

const AUDIOCPP_VERSION = 'v0.9.1'; // Apache-2.0
const LLAMA_BUILD = 'b11696'; // MIT; rolling build, release tags carry no binaries
const YTDLP_VERSION = '2026.08.19'; // Unlicense

// The VAD weights are platform-neutral: same upstream tag, same file, on both.
const SILERO_URL = `https://raw.githubusercontent.com/0xShug0/audio.cpp/${AUDIOCPP_VERSION}/assets/framework/models/silero_vad/silero_vad_16k.safetensors`;
const SILERO_SHA256 = 'c59271c284ae9c8335d795d60e0bfdb71aaaceec578d9bd9ffc1b8153c319ea1';

const YTDLP_RAW_BASE = `https://raw.githubusercontent.com/yt-dlp/yt-dlp/${YTDLP_VERSION}`;

const platform = hostPlatform();

main().catch((error) => {
    log.err(error.message);
    process.exit(1);
});

async function main() {
    ensureDir(binDir);
    ensureDir(sileroDir);
    ensureDir(licensesDir);

    if (platform === 'macos') {
        await prepareMacOs();
    } else if (platform === 'windows') {
        await prepareWindows();
    } else {
        die(`prepare-resources runs on macOS or Windows (current: ${platform})`);
    }

    await fetchSilero();
    await fetchYtDlpLicenses();
    writeManifest();
    log.ok('资源准备完毕。');
}

// ---------------------------------------------------------------------------
// macOS
// ---------------------------------------------------------------------------

async function prepareMacOs() {
    log.step('1. 清理已废弃的 Python/uv/旧引擎资源...');
    // Paths from the v1 Python/FunASR packaging. A checkout that still has them
    // would otherwise be bundled by Tauri's `resources/**/*` glob. Enumerated
    // explicitly so the fetched artifacts are never touched.
    for (const obsolete of ['python', 'python-venv', 'core-engine', 'lib', 'uv']) {
        const dir = path.join(resourcesDir, obsolete);
        if (fs.existsSync(dir)) {
            log.step(`  移除 ${obsolete}/`);
            remove(dir);
        }
    }
    for (const stale of filesWithPrefix(binDir, 'uv')) remove(stale);
    for (const stale of filesWithPrefix(binDir, 'monosub-engine-')) remove(stale);

    log.step('2. 检测 FFmpeg 二进制环境...');
    prepareFfmpegMacOs();

    log.step('3. 获取 audio.cpp 运行时 (CLI + Silero VAD)...');
    await fetchAudioCppMacOs();

    log.step('4. 获取 llama.cpp 运行时 (llama-server + dylib)...');
    await fetchLlamaCppMacOs();

    log.step('5. 获取 yt-dlp (URL 导入的字幕/音频下载器)...');
    await fetchYtDlpMacOs();
}

function prepareFfmpegMacOs() {
    const ffmpegTarget = path.join(binDir, bundledName('ffmpeg'));
    const ffprobeTarget = path.join(binDir, bundledName('ffprobe'));

    if (isExecutable(ffmpegTarget) && isExecutable(ffprobeTarget)) {
        log.ok(`FFmpeg 已存在 (${targetTriple()})，跳过`);
        return;
    }

    // `brew --prefix ffmpeg` first, then whatever is on PATH. Both are plain
    // copies: the Homebrew binaries are what the developer already runs, and the
    // dylib rewrite (bundle-ffmpeg.mjs) is a separate, opt-in step.
    const brewPrefix = tryCapture('brew', ['--prefix', 'ffmpeg']);
    const ffmpegBin = tryCapture('command', ['-v', 'ffmpeg']);
    const ffprobeBin = tryCapture('command', ['-v', 'ffprobe']);

    let ffmpegSource = null;
    let ffprobeSource = null;
    if (brewPrefix) {
        log.step('从 Homebrew 复制 FFmpeg...');
        ffmpegSource = path.join(brewPrefix, 'bin', 'ffmpeg');
        ffprobeSource = path.join(brewPrefix, 'bin', 'ffprobe');
    } else if (ffmpegBin && ffprobeBin) {
        log.step('从系统路径复制 FFmpeg...');
        ffmpegSource = ffmpegBin;
        ffprobeSource = ffprobeBin;
    }

    if (!ffmpegSource || !ffprobeSource) {
        die('未找到 FFmpeg，请先安装: brew install ffmpeg');
    }

    copyFile(ffmpegSource, ffmpegTarget);
    copyFile(ffprobeSource, ffprobeTarget);
    log.ok('FFmpeg 复制完成');
}

async function fetchAudioCppMacOs() {
    const isArm = process.arch === 'arm64';
    const asset = isArm
        ? `audio-${AUDIOCPP_VERSION}-bin-macos-arm64-metal.tar.gz`
        : `audio-${AUDIOCPP_VERSION}-bin-macos-x64-metal.tar.gz`;
    // The x64 pin has never been computed (no Intel Mac in the loop). Empty
    // means "skip verification" so a build for that target is still possible.
    const sha256 = isArm
        ? '960436787b84bf137a70ea1713ac460207ef2ac7b2617380fb7a1f4650d1100d'
        : '';

    const target = path.join(binDir, bundledName('audiocpp_cli'));
    if (isExecutable(target)) {
        log.ok(`audiocpp_cli already present, skipping (${targetTriple()})`);
        return;
    }

    const scratch = makeScratchDir('monosub-audiocpp-');
    try {
        const archive = path.join(scratch, asset);
        const url = `https://github.com/0xShug0/audio.cpp/releases/download/${AUDIOCPP_VERSION}/${asset}`;
        await downloadAndVerify(url, archive, sha256);
        extractTarGz(archive, scratch, ['audiocpp_cli']);
        copyFile(path.join(scratch, 'audiocpp_cli'), target);
        log.ok(`Installed ${target}`);
    } finally {
        remove(scratch);
    }

    run(target, ['--version']);
}

async function fetchLlamaCppMacOs() {
    const isArm = process.arch === 'arm64';
    const asset = isArm
        ? `llama-${LLAMA_BUILD}-bin-macos-arm64.tar.gz`
        : `llama-${LLAMA_BUILD}-bin-macos-x64.tar.gz`;
    const sha256 = isArm
        ? '6aac007fbe153efefe06250d2cf05d0487583aa0706379082ebef2b05579f649'
        : '';

    const target = path.join(binDir, bundledName('llama-server'));
    // "Already present" means the executable plus the rpath libraries it cannot
    // run without. The exact dylib set is an upstream detail, so the check only
    // asks for both families instead of a list that would rot on every bump.
    const dylibsPresent =
        filesWithPrefix(binDir, 'libllama').length > 0 || filesWithPrefix(binDir, 'libggml').length > 0;

    if (isExecutable(target) && dylibsPresent) {
        log.ok(`llama-server already present, skipping (${targetTriple()})`);
        return;
    }

    const scratch = makeScratchDir('monosub-llama-');
    try {
        const archive = path.join(scratch, asset);
        const url = `https://github.com/ggml-org/llama.cpp/releases/download/${LLAMA_BUILD}/${asset}`;
        await downloadAndVerify(url, archive, sha256);

        // The tarball also carries llama-cli, llama-tokenize and friends; the
        // whole archive goes to the scratch dir and only the server and the
        // shared libraries are installed, so nothing else can reach the DMG.
        extractTarGz(archive, scratch);
        const extracted = path.join(scratch, `llama-${LLAMA_BUILD}`);
        if (!fs.existsSync(path.join(extracted, 'llama-server'))) {
            throw new Error(`llama-server missing from the archive layout: ${extracted}`);
        }

        copyFile(path.join(extracted, 'llama-server'), target);

        // Why these dylibs land FLAT in bin/ and not in bin/lib/: llama-server's
        // only LC_RPATH is @loader_path, so its libraries must sit in the
        // executable's own directory. (ffmpeg's dylibs live in bin/lib/ and are
        // resolved by that binary's own rpath — the two layouts must not merge.)
        let count = 0;
        for (const entry of fs.readdirSync(extracted)) {
            if (!entry.startsWith('lib') || !entry.endsWith('.dylib')) continue;
            copyFile(path.join(extracted, entry), path.join(binDir, entry));
            count += 1;
        }
        if (count === 0) {
            throw new Error('no lib*.dylib in the archive; llama-server could not start (@loader_path)');
        }
        log.ok(`Installed ${target} (+${count} dylibs flat in bin/)`);
    } finally {
        remove(scratch);
    }

    run(target, ['--version']);
}

async function fetchYtDlpMacOs() {
    // The macOS asset is a Universal binary (arm64 + x86_64 in one file), so
    // both triples install the same download — hence one shared pin.
    await fetchYtDlpBinary('yt-dlp_macos', '0f192b7ec147ab6288885d6351d9ab67367640029b4377576ef46dd79cf7b202', 37146048);
}

// ---------------------------------------------------------------------------
// Windows
// ---------------------------------------------------------------------------

async function prepareWindows() {
    await fetchAudioCppWindows();
    await fetchLlamaCppWindows();
    copyMsvcRuntimeFlat();
    await fetchFfmpegWindows();
    await fetchYtDlpWindows();
    smokeTestWindows();
}

/** True when an engine directory holds the executable plus the CUDA runtime it cannot start without. */
function engineInstalled(dir, exe, requiredPrefixes) {
    if (!isExecutable(exe)) return false;
    for (const prefix of requiredPrefixes) {
        if (filesWithPrefix(dir, prefix).filter((file) => file.endsWith('.dll')).length === 0) {
            return false;
        }
    }
    return true;
}

async function fetchAudioCppWindows() {
    const dir = path.join(binDir, 'audiocpp_cli');
    const target = path.join(dir, bundledName('audiocpp_cli'));

    if (engineInstalled(dir, target, ['ggml-cuda', 'cublas64_13', 'cudart64_13', 'cufft64_12'])) {
        log.ok(`audiocpp_cli already present, skipping (${targetTriple()}, cuda13.3)`);
        return;
    }

    // The build archive and the cudart archive are one logical artifact: the CLI
    // cannot load ggml-cuda.dll without cuBLAS/cuFFT/cudart beside it, so a
    // partial install is treated as no install at all.
    const build = {
        asset: `audio-${AUDIOCPP_VERSION}-bin-windows-x64-cuda13.3.zip`,
        sha256: 'dd04894fae8bb0d1365b4f62407fb51550558a6478f1ef719cad7aefc00720de',
    };
    const cudart = {
        asset: `audio-${AUDIOCPP_VERSION}-cudart-windows-x64-cuda13.3.zip`,
        sha256: 'fa0fb817540c578bad085c7befb9d25b5a7634548e47955bff7b732b04b8a84b',
    };

    const base = `https://github.com/0xShug0/audio.cpp/releases/download/${AUDIOCPP_VERSION}`;
    const scratch = makeScratchDir('monosub-audiocpp-');
    try {
        ensureDir(dir);
        const extracted = path.join(scratch, 'build');
        await downloadAndExtract(`${base}/${build.asset}`, path.join(scratch, build.asset), build.sha256, extracted);
        copyFile(path.join(extracted, 'audiocpp_cli.exe'), target);
        const buildDlls = copyAllDlls(extracted, dir);
        if (buildDlls === 0) throw new Error('no DLLs in the audio.cpp archive; the CLI cannot start');

        // Every DLL from the build: ggml.dll, ggml-base.dll, ggml-cuda.dll and
        // the whole ggml-cpu-<microarch> family (ggml picks one at load time),
        // plus the MSVC runtime.
        const runtimeDir = path.join(scratch, 'cudart');
        await downloadAndExtract(`${base}/${cudart.asset}`, path.join(scratch, cudart.asset), cudart.sha256, runtimeDir);
        const runtimeDlls = copyAllDlls(runtimeDir, dir);
        if (runtimeDlls === 0) throw new Error('no DLLs in the audio.cpp cudart archive');

        log.ok(`Installed ${target} (+${buildDlls} build DLLs, +${runtimeDlls} CUDA runtime DLLs)`);
    } finally {
        remove(scratch);
    }
}

async function fetchLlamaCppWindows() {
    const dir = path.join(binDir, 'llama-server');
    const target = path.join(dir, bundledName('llama-server'));

    if (engineInstalled(dir, target, ['ggml-cuda', 'llama-server-impl', 'cublas64_13', 'cudart64_13'])) {
        log.ok(`llama-server already present, skipping (${targetTriple()}, cuda13.4)`);
        return;
    }

    // CUDA 13.4 is the closest track to audio.cpp's 13.3; they stay in separate
    // directories precisely so the mismatch cannot matter.
    const build = {
        asset: `llama-${LLAMA_BUILD}-bin-win-cuda-13.4-x64.zip`,
        sha256: 'baea0383b4e040f601a6091348b0508531f77f6756bf7dec6689881470fd4614',
    };
    const cudart = {
        asset: `cudart-llama-bin-win-cuda-13.4-x64.zip`,
        sha256: '738f8c251ac22b70c3ae6f83a10cf222725df0395246a2cf58f32bdb85fbe668',
    };

    const base = `https://github.com/ggml-org/llama.cpp/releases/download/${LLAMA_BUILD}`;
    const scratch = makeScratchDir('monosub-llama-');
    try {
        ensureDir(dir);
        const extracted = path.join(scratch, 'build');
        await downloadAndExtract(`${base}/${build.asset}`, path.join(scratch, build.asset), build.sha256, extracted);
        copyFile(path.join(extracted, 'llama-server.exe'), target);

        // Only the server is installed, but every DLL is: it is a thin launcher
        // over llama-server-impl.dll, which pulls in llama-common, llama, mtmd,
        // libomp and the whole ggml family including ggml-cuda.
        const buildDlls = copyAllDlls(extracted, dir);
        if (buildDlls === 0) throw new Error('no DLLs in the llama archive; llama-server cannot start');

        const runtimeDir = path.join(scratch, 'cudart');
        await downloadAndExtract(`${base}/${cudart.asset}`, path.join(scratch, cudart.asset), cudart.sha256, runtimeDir);
        const runtimeDlls = copyAllDlls(runtimeDir, dir);
        if (runtimeDlls === 0) throw new Error('no DLLs in the llama cudart archive');

        log.ok(`Installed ${target} (+${buildDlls} build DLLs, +${runtimeDlls} CUDA runtime DLLs)`);
    } finally {
        remove(scratch);
    }
}

/**
 * ffmpeg, ffprobe, yt-dlp and monosub-cli live directly in bin\ and are resolved
 * from there, so the MSVC runtime has to sit beside them too — the copies inside
 * each engine directory serve the engines, these serve everything else.
 */
function copyMsvcRuntimeFlat() {
    const audioCppDir = path.join(binDir, 'audiocpp_cli');
    let count = 0;
    for (const prefix of ['msvcp140', 'vcruntime140']) {
        for (const dll of filesWithPrefix(audioCppDir, prefix).filter((file) => file.endsWith('.dll'))) {
            copyFile(dll, path.join(binDir, path.basename(dll)));
            count += 1;
        }
    }
    if (count > 0) log.ok(`MSVC runtime DLLs staged flat in bin\\ (${count})`);
}

async function fetchFfmpegWindows() {
    const ffmpegTarget = path.join(binDir, bundledName('ffmpeg'));
    const ffprobeTarget = path.join(binDir, bundledName('ffprobe'));
    if (isExecutable(ffmpegTarget) && isExecutable(ffprobeTarget)) {
        log.ok(`ffmpeg already present, skipping (${targetTriple()})`);
        return;
    }

    // gyan.dev re-points the version-less URL on every release, so the pinned
    // file name in `packages/` is what keeps this reproducible.
    const asset = 'ffmpeg-8.1.2-essentials_build.zip';
    const sha256 = 'db580001caa24ac104c8cb856cd113a87b0a443f7bdf47d8c12b1d740584a2ec';
    const url = `https://www.gyan.dev/ffmpeg/builds/packages/${asset}`;

    const scratch = makeScratchDir('monosub-ffmpeg-');
    try {
        const extracted = path.join(scratch, 'ffmpeg');
        await downloadAndExtract(url, path.join(scratch, asset), sha256, extracted);

        const binFolder = findDir(extracted, 'bin', 3);
        if (!binFolder) throw new Error('unexpected ffmpeg archive layout: no bin directory');

        // The essentials build is static: a single .exe, no lib\*.dll to ship.
        copyFile(path.join(binFolder, 'ffmpeg.exe'), ffmpegTarget);
        copyFile(path.join(binFolder, 'ffprobe.exe'), ffprobeTarget);
        log.ok(`Installed ${ffmpegTarget}, ${ffprobeTarget}`);
    } finally {
        remove(scratch);
    }
}

async function fetchYtDlpWindows() {
    await fetchYtDlpBinary(
        'yt-dlp.exe',
        '66674953fe251b89f4d08c5f0e35e0728679bd67ab3d7d05c0562af101dd3e7a',
        0
    );
}

/**
 * `--version` only proves the DLL graph loads. Say so when the machine has no
 * NVIDIA GPU, because that is the one remaining reason recognition would fail at
 * run time and it is invisible to every check above.
 */
function smokeTestWindows() {
    log.step('Verifying bundled binaries ...');
    run(path.join(binDir, 'audiocpp_cli', bundledName('audiocpp_cli')), ['--version']);
    run(path.join(binDir, 'llama-server', bundledName('llama-server')), ['--version']);
    run(path.join(binDir, bundledName('ffmpeg')), ['-version']);
    run(path.join(binDir, bundledName('yt-dlp')), ['--version']);

    if (nvidiaAdapterCount().trim() === '0') {
        log.warn('No NVIDIA display adapter found: the CUDA engines load, but recognition will');
        log.warn('fail at run time on this machine. Windows ships CUDA only — there is no CPU fallback.');
    }
}

// ---------------------------------------------------------------------------
// Shared
// ---------------------------------------------------------------------------

async function fetchYtDlpBinary(asset, sha256, expectedSize) {
    const target = path.join(binDir, bundledName('yt-dlp'));
    // "Already present" means present AND matching the pinned digest. A stale
    // binary from an older pin must be replaced, not silently kept, or the pin
    // stops describing what ships.
    if (matchesSha256(target, sha256)) {
        log.ok(`yt-dlp already present, skipping (${targetTriple()})`);
        return;
    }

    const url = `https://github.com/yt-dlp/yt-dlp/releases/download/${YTDLP_VERSION}/${asset}`;
    const scratch = makeScratchDir('monosub-ytdlp-');
    try {
        const downloaded = path.join(scratch, asset);
        await download(url, downloaded);
        assertSha256(downloaded, sha256, asset);

        if (expectedSize > 0) {
            const actual = fileSize(downloaded);
            if (actual !== expectedSize) {
                throw new Error(`unexpected size for ${asset}: ${actual} (expected ${expectedSize})`);
            }
        }
        copyFile(downloaded, target);
        log.ok(`Installed ${target}`);
    } finally {
        remove(scratch);
    }

    run(target, ['--version']);
}

/**
 * yt-dlp is Unlicense, but it statically bundles third-party code, so both
 * files ship with the app (AGENTS.md → Licensing / Attribution).
 */
async function fetchYtDlpLicenses() {
    for (const [remote, local] of [
        ['LICENSE', 'yt-dlp-LICENSE'],
        ['THIRD_PARTY_LICENSES.txt', 'yt-dlp-THIRD_PARTY_LICENSES.txt'],
    ]) {
        const target = path.join(licensesDir, local);
        if (fs.existsSync(target) && fileSize(target) > 0) {
            log.ok(`${local} already present, skipping`);
            continue;
        }
        await download(`${YTDLP_RAW_BASE}/${remote}`, target);
    }
}

async function fetchSilero() {
    const target = path.join(sileroDir, 'silero_vad_16k.safetensors');
    if (matchesSha256(target, SILERO_SHA256)) {
        log.ok('silero_vad_16k.safetensors already present, skipping');
        return;
    }
    const scratch = makeScratchDir('monosub-silero-');
    try {
        const downloaded = path.join(scratch, 'silero.safetensors');
        await download(SILERO_URL, downloaded);
        assertSha256(downloaded, SILERO_SHA256, 'silero_vad_16k.safetensors');
        copyFile(downloaded, target, 0o644);
        log.ok(`Installed ${target}`);
    } finally {
        remove(scratch);
    }
}

/**
 * MANIFEST.txt records what the build actually bundles. Generated rather than
 * hand-maintained so it cannot drift; no timestamp, so re-runs are byte-identical.
 * Same file on both platforms on purpose — there is one `resources\` directory
 * and it lists whatever is present.
 */
function writeManifest() {
    const lines = [
        '# MonoSub 资源清单',
        '# 由 scripts/release/prepare-resources.mjs 生成，请勿手工编辑。',
        '',
        '## 文件',
    ];
    for (const rel of listFiles(resourcesDir, { exclude: ['MANIFEST.txt'] })) {
        const size = fileSize(path.join(resourcesDir, ...rel.split('/')));
        lines.push(`${rel.padEnd(56)} ${size}`);
    }
    writeTextFile(manifestPath, `${lines.join('\n')}\n`);
    log.ok(`已写入 ${manifestPath} (${humanSize(fileSize(manifestPath))})`);
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

async function downloadAndVerify(url, archive, sha256) {
    await download(url, archive);
    assertSha256(archive, sha256, path.basename(archive));
    return archive;
}

async function downloadAndExtract(url, archive, sha256, destination) {
    await download(url, archive);
    assertSha256(archive, sha256, path.basename(archive));
    log.step(`Extracting ${path.basename(archive)} ...`);
    await extractZip(archive, destination);
    return destination;
}

function copyAllDlls(from, to) {
    let count = 0;
    for (const entry of fs.readdirSync(from, { withFileTypes: true })) {
        if (!entry.isFile() || !entry.name.endsWith('.dll')) continue;
        copyFile(path.join(from, entry.name), path.join(to, entry.name));
        count += 1;
    }
    return count;
}

/** First directory named `name` within `maxDepth` levels of `root`. */
function findDir(root, name, maxDepth) {
    const walk = (dir, depth) => {
        for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
            if (!entry.isDirectory()) continue;
            const full = path.join(dir, entry.name);
            if (entry.name === name) return full;
            if (depth < maxDepth) {
                const found = walk(full, depth + 1);
                if (found) return found;
            }
        }
        return null;
    };
    return walk(root, 1);
}

/** NVIDIA adapter count via CIM; returns '0' when PowerShell is unavailable. */
function nvidiaAdapterCount() {
    const output = tryCapture('powershell', [
        '-NoProfile',
        '-Command',
        "(Get-CimInstance Win32_VideoController -ErrorAction SilentlyContinue | Where-Object { $_.Name -match 'NVIDIA' } | Measure-Object).Count",
    ]);
    return output ?? '0';
}
