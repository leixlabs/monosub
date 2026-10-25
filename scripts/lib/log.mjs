// Shared logging.
//
// One implementation for both platforms. Colour is emitted only when stdout is
// a TTY, so a captured CI log stays plain text — the previous bash scripts
// hard-coded escapes, which is why their logs carried `^[[0;34m` noise whenever
// output was piped.
//
// Windows note: Windows Terminal, PowerShell 7 and ConPTY all render ANSI
// colour; legacy cmd.exe does not, and there `process.stdout.isTTY` is false
// anyway, so the same guard covers it.

const RESET = '[0m';
const BLUE = '[0;34m';
const GREEN = '[0;32m';
const RED = '[0;31m';
const YELLOW = '[1;33m';
const DIM = '[2m';

const useColor = Boolean(process.stdout.isTTY) && !process.env.NO_COLOR;

function paint(code, text) {
    return useColor ? `${code}${text}${RESET}` : text;
}

/** `log.*` writes to stdout except `err`, which belongs on stderr. */
export const log = {
    info(message) {
        console.log(`${paint(BLUE, '[INFO]')} ${message}`);
    },
    step(message) {
        console.log(`${paint(BLUE, '[INFO]')} ${message}`);
    },
    ok(message) {
        console.log(`${paint(GREEN, '[OK]')} ${message}`);
    },
    success(message) {
        console.log(`${paint(GREEN, '[SUCCESS]')} ${message}`);
    },
    warn(message) {
        console.log(`${paint(YELLOW, '[WARNING]')} ${message}`);
    },
    err(message) {
        console.error(`${paint(RED, '[ERROR]')} ${message}`);
    },
    dim(message) {
        console.log(paint(DIM, message));
    },
    plain(message) {
        console.log(message);
    },
    blank() {
        console.log('');
    },
};

/** Print `message` as an error and exit non-zero. */
export function die(message) {
    log.err(message);
    process.exit(1);
}

/** The `=====` banner the release/publish flows open with. */
export function header(title) {
    const rule = '='.repeat(46);
    console.log(rule);
    console.log(title);
    console.log(rule);
}

/** Human-readable size, used when listing the artifacts in dist/. */
export function humanSize(bytes) {
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
    if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
    return `${(bytes / (1024 * 1024 * 1024)).toFixed(2)} GB`;
}
