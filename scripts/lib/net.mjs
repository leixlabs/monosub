// Pinned downloads and checksums.
//
// Every artifact these scripts install is pinned by SHA-256 and fetched with
// retries. The retries are not decoration: the yt-dlp and GitHub release CDNs
// regularly drop a connection mid-transfer, and a truncated archive that is
// still installed would ship — which is exactly what the checksum is there to
// catch, but a checksum failure at the end of a 600 MB download is a worse
// outcome than retrying.

import fs from 'node:fs';
import fsp from 'node:fs/promises';
import path from 'node:path';
import { createHash } from 'node:crypto';

import { log, humanSize } from './log.mjs';
import { sleep } from './fs.mjs';

/**
 * Download `url` to `destination`.
 *
 * `fetch` follows the redirects GitHub release assets use (→ objects.
 * githubusercontent.com), and the body is streamed to disk so a 600 MB CUDA
 * archive never lands in memory.
 */
export async function download(url, destination, { retries = 5, label } = {}) {
    const name = label ?? path.basename(destination);
    fs.mkdirSync(path.dirname(destination), { recursive: true });

    let lastError = null;
    for (let attempt = 1; attempt <= retries; attempt++) {
        const handle = await fsp.open(destination, 'w');
        try {
            const response = await fetch(url);
            if (!response.ok) {
                throw new Error(`HTTP ${response.status} ${response.statusText}`);
            }
            if (!response.body) {
                throw new Error('empty response body');
            }

            let written = 0;
            for await (const chunk of response.body) {
                await handle.write(chunk);
                written += chunk.length;
            }
            await handle.close();

            log.ok(`Downloaded ${name} (${humanSize(written)})`);
            return destination;
        } catch (error) {
            await handle.close().catch(() => {});
            await fsp.rm(destination, { force: true });
            lastError = error;
            if (attempt < retries) {
                log.warn(`${name}: ${error.message} — retrying (${attempt}/${retries - 1})`);
                await sleep(2000 * attempt);
            }
        }
    }
    throw new Error(`failed to download ${url}: ${lastError?.message ?? 'unknown error'}`);
}

/** SHA-256 of a file, streamed. Lower-case hex, like `shasum -a 256`. */
export async function sha256File(file) {
    const hash = createHash('sha256');
    const handle = await fsp.open(file, 'r');
    try {
        const buffer = Buffer.alloc(1024 * 1024);
        let read = 0;
        while ((read = (await handle.read(buffer, 0, buffer.length, null)).bytesRead) > 0) {
            hash.update(buffer.subarray(0, read));
        }
    } finally {
        await handle.close();
    }
    return hash.digest('hex');
}

export function sha256FileSync(file) {
    return createHash('sha256').update(fs.readFileSync(file)).digest('hex');
}

/**
 * Throw when `file` does not match `expected`.
 *
 * An empty `expected` means "not pinned yet" and skips — the macOS x64 assets
 * were pinned later than arm64, and a build for an un-pinned target must still
 * be possible rather than blocked by a checksum nobody has computed.
 */
export function assertSha256(file, expected, label) {
    if (!expected) return;
    const actual = sha256FileSync(file);
    if (actual !== expected.toLowerCase()) {
        const name = label ?? path.basename(file);
        log.err(`Checksum mismatch for ${name}`);
        log.err(`  expected ${expected}`);
        log.err(`  actual   ${actual}`);
        throw new Error(`checksum mismatch for ${name}`);
    }
}

/** True when `file` exists and matches `expected` (or the pin is empty). */
export function matchesSha256(file, expected) {
    if (!fs.existsSync(file)) return false;
    if (!expected) return true;
    return sha256FileSync(file) === expected.toLowerCase();
}
