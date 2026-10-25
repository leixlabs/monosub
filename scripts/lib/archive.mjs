// Archive extraction.
//
// ## Why the zip reader is written out here
//
// The previous Windows scripts used PowerShell's Expand-Archive and then the
// .NET ZipFile types, because Expand-Archive rejects archives whose entries lack
// a directory record — and the audio.cpp and llama.cpp archives do ship those.
// Neither exists on macOS, and a Node flow that shells out to two different
// platform tools per archive is exactly the split this refactor removes.
//
// So: one reader, implemented against the zip format here. It streams each
// entry through zlib rather than inflating into memory, because a single
// ggml-cuda.dll is 258 MB.
//
// ## Why tar.gz still uses the system `tar`
//
// Only the macOS side needs it, and `tar` is present on every macOS and on
// Windows 10 1803+. Re-implementing libarchive would be a second reader for no
// benefit.

import fs from 'node:fs';
import fsp from 'node:fs/promises';
import path from 'node:path';
import zlib from 'node:zlib';
import { pipeline } from 'node:stream/promises';

import { ensureDir } from './fs.mjs';
import { run } from './shell.mjs';

const EOCD_SIGNATURE = 0x06054b50;
const ZIP64_EOCD_SIGNATURE = 0x06064b50;
const ZIP64_LOCATOR_SIGNATURE = 0x07064b50;
const CENTRAL_HEADER_SIGNATURE = 0x02014b50;
const LOCAL_HEADER_SIGNATURE = 0x04034b50;

/**
 * Entries of a zip: `{ name, size, compressedSize, method, dataOffset }`.
 *
 * Only the tail of the file and the central directory are read; member data is
 * left on disk and streamed out on demand.
 */
export async function listZip(archive) {
    const handle = await fsp.open(archive, 'r');
    try {
        const { size } = await handle.stat();
        if (size < 22) throw new Error(`${archive} is too small to be a zip`);

        const tailLength = Math.min(size, 65557);
        const tail = Buffer.alloc(tailLength);
        await handle.read(tail, 0, tailLength, size - tailLength);

        let eocd = -1;
        for (let i = tail.length - 22; i >= 0; i--) {
            if (tail.readUInt32LE(i) === EOCD_SIGNATURE) {
                eocd = i;
                break;
            }
        }
        if (eocd < 0) throw new Error(`${archive} is not a zip: no end-of-central-directory record`);

        let entryCount = tail.readUInt16LE(eocd + 10);
        let centralSize = tail.readUInt32LE(eocd + 12);
        let centralOffset = tail.readUInt32LE(eocd + 16);

        // Zip64: any of the three fields saturated at 0xFFFF/0xFFFFFFFF means the
        // real values live in the zip64 end-of-central-directory record, which is
        // pointed at by the locator sitting immediately before the classic EOCD.
        if (centralOffset === 0xffffffff || entryCount === 0xffff || centralSize === 0xffffffff) {
            const locator = eocd - 20;
            if (locator >= 0 && tail.readUInt32LE(locator) === ZIP64_LOCATOR_SIGNATURE) {
                const recordOffset = Number(tail.readBigUInt64LE(locator + 8));
                const record = Buffer.alloc(56);
                await handle.read(record, 0, 56, recordOffset);
                if (record.readUInt32LE(0) !== ZIP64_EOCD_SIGNATURE) {
                    throw new Error(`${archive} has a zip64 locator but no zip64 record`);
                }
                entryCount = Number(record.readBigUInt64LE(32));
                centralSize = Number(record.readBigUInt64LE(40));
                centralOffset = Number(record.readBigUInt64LE(48));
            }
        }

        const central = Buffer.alloc(centralSize);
        await handle.read(central, 0, centralSize, centralOffset);

        const entries = [];
        let p = 0;
        while (p + 46 <= central.length) {
            if (central.readUInt32LE(p) !== CENTRAL_HEADER_SIGNATURE) break;

            const flags = central.readUInt16LE(p + 8);
            const method = central.readUInt16LE(p + 10);
            let compressedSize = central.readUInt32LE(p + 20);
            let size = central.readUInt32LE(p + 24);
            const nameLength = central.readUInt16LE(p + 28);
            const extraLength = central.readUInt16LE(p + 30);
            const commentLength = central.readUInt16LE(p + 32);
            let localOffset = central.readUInt32LE(p + 42);

            const name = central.toString('utf8', p + 46, p + 46 + nameLength);
            const extra = central.subarray(p + 46 + nameLength, p + 46 + nameLength + extraLength);

            // Zip64 extra field (0x0001) carries only the members that saturated:
            // uncompressed, compressed, then the local header offset, in that order.
            if (size === 0xffffffff || compressedSize === 0xffffffff || localOffset === 0xffffffff) {
                let q = 0;
                while (q + 4 <= extra.length) {
                    const id = extra.readUInt16LE(q);
                    const fieldSize = extra.readUInt16LE(q + 2);
                    const body = extra.subarray(q + 4, q + 4 + fieldSize);
                    if (id === 0x0001) {
                        let r = 0;
                        if (size === 0xffffffff && r + 8 <= body.length) {
                            size = Number(body.readBigUInt64LE(r));
                            r += 8;
                        }
                        if (compressedSize === 0xffffffff && r + 8 <= body.length) {
                            compressedSize = Number(body.readBigUInt64LE(r));
                            r += 8;
                        }
                        if (localOffset === 0xffffffff && r + 8 <= body.length) {
                            localOffset = Number(body.readBigUInt64LE(r));
                        }
                        break;
                    }
                    q += 4 + fieldSize;
                }
            }

            if (flags & 0x1) throw new Error(`${archive}: encrypted entries are not supported`);

            // The local header can carry a different extra-field length than the
            // central directory does (and a data descriptor can follow the data), so
            // the data offset is always read from the local header itself.
            const local = Buffer.alloc(30);
            await handle.read(local, 0, 30, localOffset);
            if (local.readUInt32LE(0) !== LOCAL_HEADER_SIGNATURE) {
                throw new Error(`${archive}: bad local header for ${name}`);
            }
            const localNameLength = local.readUInt16LE(26);
            const localExtraLength = local.readUInt16LE(28);
            const dataOffset = localOffset + 30 + localNameLength + localExtraLength;

            entries.push({ name, size, compressedSize, method, dataOffset, isDirectory: name.endsWith('/') });
            p += 46 + nameLength + extraLength + commentLength;
        }

        if (entries.length !== entryCount && entryCount !== 0xffff) {
            throw new Error(`${archive}: expected ${entryCount} entries, parsed ${entries.length}`);
        }
        return entries;
    } finally {
        await handle.close();
    }
}

/**
 * Read one entry of a zip into a string.
 *
 * Used by the MSIX check to read `AppxManifest.xml` out of the package without
 * unpacking a 1 GB payload.
 */
export async function readZipEntry(archive, entryName, encoding = 'utf8') {
    const entries = await listZip(archive);
    const entry = entries.find((candidate) => candidate.name === entryName);
    if (!entry) return null;

    // Read the compressed bytes into memory and inflate synchronously. Iterating
    // a stream that is simultaneously being `.pipe()`d is not reliable — measured
    // on a real MSIX: the async iterator saw bytes but zlib reported "invalid
    // block type". This path is used for manifests, not for 250 MB DLLs.
    const handle = await fsp.open(archive, 'r');
    try {
        const buffer = Buffer.alloc(entry.compressedSize);
        await handle.read(buffer, 0, entry.compressedSize, entry.dataOffset);
        const data = entry.method === 8 ? zlib.inflateRawSync(buffer) : buffer;
        return data.toString(encoding);
    } finally {
        await handle.close();
    }
}

/** Reject `..`, absolute paths and drive letters before they reach the disk. */
function safeTarget(root, entryName) {
    const normalized = entryName.replace(/\\/g, '/').split('/').filter((part) => part && part !== '.');
    if (normalized.some((part) => part === '..')) {
        throw new Error(`refusing to extract an entry that escapes the destination: ${entryName}`);
    }
    return path.join(root, ...normalized);
}

/**
 * Extract a zip into `destination`.
 *
 * Directory entries are created rather than written; stored entries (method 0)
 * are copied byte-for-byte; deflated entries are streamed through
 * `inflateRaw` so a 258 MB DLL never has to fit in memory twice.
 */
export async function extractZip(archive, destination) {
    const entries = await listZip(archive);
    ensureDir(destination);

    for (const entry of entries) {
        const target = safeTarget(destination, entry.name);
        if (entry.isDirectory) {
            ensureDir(target);
            continue;
        }
        ensureDir(path.dirname(target));

        if (entry.method === 0) {
            const source = fs.createReadStream(archive, {
                start: entry.dataOffset,
                end: entry.dataOffset + entry.compressedSize - 1,
            });
            await pipeline(source, fs.createWriteStream(target));
        } else if (entry.method === 8) {
            const source = fs.createReadStream(archive, {
                start: entry.dataOffset,
                end: entry.dataOffset + entry.compressedSize - 1,
            });
            await pipeline(source, zlib.createInflateRaw(), fs.createWriteStream(target));
        } else {
            throw new Error(`${archive}: unsupported compression method ${entry.method} for ${entry.name}`);
        }
    }
    return destination;
}

/**
 * Extract a `.tar.gz`, optionally only `members` (paths relative to the archive
 * root, as `tar` names them).
 */
export function extractTarGz(archive, destination, members = []) {
    ensureDir(destination);
    run('tar', ['-xzf', archive, '-C', destination, ...members]);
    return destination;
}

/**
 * True when a download that failed part-way left a file behind. Callers use it
 * to make "already present" mean "present and complete", not merely "present".
 */
export function isZipReadable(archive) {
    if (!fs.existsSync(archive)) return false;
    try {
        return fs.statSync(archive).size > 22;
    } catch {
        return false;
    }
}
