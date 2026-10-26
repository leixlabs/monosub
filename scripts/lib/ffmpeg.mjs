// Shared FFmpeg helpers: license verification for the binaries we bundle.
//
// MonoSub is closed source and sold through the Mac App Store and Microsoft
// Store, so we must never ship an FFmpeg binary configured with `--enable-gpl`
// or `--enable-nonfree`. With `--enable-gpl` the *whole* binary falls under the
// GPL (FFmpeg's own legal page says as much), which would force us to either
// publish Corresponding Source we do not ship (§6) or breach the licence's
// ban on further restrictions (§10, which Apple's EULA imposes). Building
// without those flags keeps FFmpeg under the LGPL, whose copyleft reaches only
// FFmpeg itself — not the app that runs it as a separate process.

import { tryCapture } from './shell.mjs';

/** Configure flags that must not appear in a binary we ship. */
const FORBIDDEN_FLAGS = ['--enable-gpl', '--enable-nonfree'];

/**
 * Read the configure line `ffmpeg -version` prints and reject forbidden flags.
 * This inspects the artifact rather than trusting the download URL, so it also
 * covers the macOS path where the binary is copied off the developer's system.
 *
 * @param {string} binary Path to an ffmpeg or ffprobe executable.
 * @throws {Error} When the configuration cannot be read, or a forbidden flag is present.
 */
export function verifyFfmpegLicense(binary) {
    const output = tryCapture(binary, ['-hide_banner', '-version']);
    const configuration = (output ?? '')
        .split('\n')
        .find((line) => line.startsWith('configuration:'));

    if (!configuration) {
        throw new Error(`cannot read the FFmpeg configuration for ${binary}`);
    }

    for (const flag of FORBIDDEN_FLAGS) {
        // Match whole flags only: `--enable-gpl` also appears inside
        // `--disable-libx264`-style strings and as a substring of longer flags.
        if (new RegExp(`(^|\\s)${flag}(\\s|$)`).test(configuration)) {
            throw new Error(
                `${binary} was built with ${flag}. That makes the whole binary ` +
                    'GPL/nonfree, which cannot be bundled in this app. Delete it ' +
                    '(it may be a leftover from before the LGPL switch) and re-run ' +
                    'to fetch an LGPL build.'
            );
        }
    }

    return true;
}
