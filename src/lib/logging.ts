import { invoke } from '@tauri-apps/api/core';

/**
 * The frontend half of "the user can find the log".
 *
 * In a release build the WebView console does not exist, so a failure that only
 * reached `console.error` is gone by the time anyone looks. Two pieces here:
 *
 * * [`reportFrontendError`] writes a core-flow failure into the same rolling
 *   file the Rust side writes (`log_frontend_error` in
 *   `src-tauri/src/commands/logs.rs`), and
 * * [`getLogDirectory`] / [`openLogsDirectory`] are what Settings ▸ System shows.
 *
 * `console` is deliberately *not* patched and `console.error` is not redirected
 * wholesale: only the flows listed in `App.tsx`, `transcription.ts`,
 * `urlImport.ts`, `Settings.tsx` and the two global hooks installed by
 * `installGlobalErrorHandlers` report. A blanket redirect floods the log with
 * React and library noise, which is how a log stops being readable.
 */

export type FrontendLogLevel = 'error' | 'warn' | 'info';

/** Same message inside this window is dropped (a burst must not flood the file). */
const DEDUPE_WINDOW_MS = 5000;

/** Upper bound on the dedupe map, so a long session cannot grow it forever. */
const MAX_TRACKED_MESSAGES = 50;

/**
 * Hard cap on what crosses IPC. The Rust side truncates as well; doing it here
 * too keeps a stack trace from becoming a multi-megabyte payload.
 */
const MAX_MESSAGE_CHARS = 4000;
const MAX_CONTEXT_CHARS = 1000;

const lastReportedAt = new Map<string, number>();

/**
 * Per-`level:message` rate limit. Returns true when this occurrence should be
 * sent, and records it.
 *
 * Keyed on the message (not the context) on purpose: the same failure in a
 * loop produces the same message with a different task id, and that is exactly
 * the storm worth collapsing.
 */
const beginReport = (key: string, now: number): boolean => {
  const previous = lastReportedAt.get(key);
  if (previous !== undefined && now - previous < DEDUPE_WINDOW_MS) return false;

  if (lastReportedAt.size >= MAX_TRACKED_MESSAGES) {
    // Drop what has aged out first: those keys could report again anyway.
    for (const [tracked, at] of lastReportedAt) {
      if (now - at >= DEDUPE_WINDOW_MS) lastReportedAt.delete(tracked);
    }
    // Still at the bound: drop the oldest entry. Map iterates in insertion order,
    // so the first key is the one that has been tracked longest.
    const oldest = lastReportedAt.keys().next();
    if (!oldest.done) lastReportedAt.delete(oldest.value);
  }

  lastReportedAt.set(key, now);
  return true;
};

/**
 * Truncate on a character boundary.
 *
 * `String#slice` counts UTF-16 units, so it can cut a surrogate pair in half and
 * leave a lone surrogate — which would then fail to cross IPC as UTF-8. Taking
 * whole characters avoids that.
 */
const truncate = (value: string, maxChars: number): string => {
  const chars = Array.from(value);
  return chars.length <= maxChars ? value : `${chars.slice(0, maxChars).join('')}…`;
};

/**
 * Turn anything thrown into a string worth reading.
 *
 * A non-`Error` rejection reason (`reject({ code })`, or `Promise.reject(null)`)
 * would otherwise become `[object Object]`, which loses exactly the detail that
 * made it worth reporting.
 */
const describeReason = (reason: unknown): string => {
  // The stack keeps the call site, which is the only part a log reader can act
  // on; it already begins with the message.
  if (reason instanceof Error) return reason.stack || reason.message || reason.name;
  if (typeof reason === 'string') return reason;
  if (reason === null || reason === undefined) return String(reason);
  try {
    return JSON.stringify(reason) ?? String(reason);
  } catch {
    return String(reason);
  }
};

/**
 * Collapse to one line.
 *
 * The log is line-oriented (one event per line, prefixed with the timestamp and
 * the source location), so an embedded newline from a stack trace would break
 * every following line. Newlines become `⏎` — still visible as a frame break,
 * and the whole report stays greppable.
 */
const oneLine = (value: string): string => value.replace(/\s*\n\s*/g, ' ⏎ ').replace(/\s+/g, ' ').trim();

/**
 * Write one core-flow failure into the application log.
 *
 * Never throws and never awaits: callers are error paths, where a logging
 * failure must not turn into a second failure. A missing backend (browser-only
 * `bun run dev`, or a command that is not registered) is swallowed.
 */
export const reportFrontendError = (
  message: unknown,
  options: { level?: FrontendLogLevel; context?: string } = {},
): void => {
  const level: FrontendLogLevel = options.level ?? 'error';
  const raw = oneLine(describeReason(message));
  const text = truncate(raw || 'unknown frontend error', MAX_MESSAGE_CHARS);
  const context = options.context ? truncate(oneLine(options.context), MAX_CONTEXT_CHARS) : null;

  if (!beginReport(`${level}:${text}`, Date.now())) return;

  try {
    void invoke('log_frontend_error', { level, message: text, context }).catch(() => {
      // Nothing to report to: this is the reporting path itself.
    });
  } catch {
    // `invoke` throws synchronously when no Tauri runtime is present.
  }
};

/**
 * Install `window.onerror` / `unhandledrejection` hooks.
 *
 * This is the only place that catches errors nobody else handled — a React
 * render error, a rejection from an un-awaited promise. Both are exactly the
 * failures that are otherwise invisible in a release build. Returns an
 * uninstall function so a test or a future hot-reload can undo it.
 */
export const installGlobalErrorHandlers = (): (() => void) => {
  const previousOnError = window.onerror;

  // `window.onerror` (rather than an `error` listener) fires only for uncaught
  // script errors, so a failing <img> or font does not report itself here.
  window.onerror = (message, source, lineno, colno, error) => {
    reportFrontendError(error ?? message, {
      level: 'error',
      context: ['uncaught', source, `${lineno}:${colno}`]
        .filter((part) => Boolean(part))
        .join(' '),
    });
    return false;
  };

  const onUnhandledRejection = (event: PromiseRejectionEvent) => {
    reportFrontendError(event.reason, {
      level: 'error',
      context: 'unhandledrejection',
    });
  };
  window.addEventListener('unhandledrejection', onUnhandledRejection);

  return () => {
    window.onerror = previousOnError;
    window.removeEventListener('unhandledrejection', onUnhandledRejection);
  };
};

/** Absolute path of the directory holding the rolling log files. */
export const getLogDirectory = (): Promise<string> => invoke<string>('log_directory');

/** Open the log directory in the system file manager. */
export const openLogsDirectory = (): Promise<void> => invoke<void>('open_logs_dir');
