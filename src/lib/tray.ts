/**
 * The menu-bar (tray) item's contents.
 *
 * The tray answers "what is the app doing?": a left click opens a menu whose
 * first rows are the current transcription progress, and the menu-bar title
 * carries the percentage while a run is in flight. The Rust side
 * (`commands/tray.rs`) owns the layout — at most three rows, truncation, the
 * separator, Show / Hide / Quit. Everything else lives here:
 *
 * - **The wording** is built from the same `steps.*` keys the window uses (plus the
 *   `tray.*` ones for the rows themselves), so the menu cannot drift into a
 *   second vocabulary and the two locales stay in sync through
 *   `check-i18n-keys.mjs`.
 * - **What counts as "current progress"** is a store question: the steps of the
 *   files that are `processing`, plus how many are queued behind them.
 *
 * The push is a state mirror, not an event stream: `startTraySync` re-reads the
 * store and `invoke`s once per change, coalesced and de-duplicated. Re-rendering
 * the menu runs on the main thread, and a transcription emits many
 * `transcribe_*` events per second.
 */
import { invoke } from '@tauri-apps/api/core';
import i18n from '@/i18n';
import { useStore, type VideoFile } from '@/store/useStore';

/** How many progress rows the menu shows. Mirrors `commands::tray::MAX_STATUS_ROWS`. */
export const MAX_TRAY_ROWS = 3;

/** Coalescing window; a burst of progress events collapses into one menu render. */
const PUSH_DELAY_MS = 250;

const t = (key: string, options?: Record<string, unknown>) => i18n.t(key, options);

/** Resolve a key, falling back to `fallback` when the locale has no such key. */
const tOr = (key: string, fallback: string, options?: Record<string, unknown>) =>
  i18n.t(key, { ...options, defaultValue: fallback });

/**
 * The i18n key for a backend step name.
 *
 * Same rule as `FileItem.tsx`: the Rust side emits a frozen English name
 * (`Extracting audio`) and the key is its lowercase, spaces → underscores.
 */
const stepKey = (stepName: string) => `steps.${stepName.toLowerCase().replace(/\s+/g, '_')}`;

const formatPercent = (value: number) =>
  `${Math.max(0, Math.min(100, Math.round(Number.isFinite(value) ? value : 0)))}%`;

/** One row for a processing file: its name, step label and step counter. */
const statusRow = (file: VideoFile): string => {
  if (file.waiting) {
    // Parked behind the concurrency limit. Saying "启动中…" here would claim
    // work that has not begun; the menu's whole job is to answer "what is the
    // app doing?" honestly, and the answer is "waiting its turn".
    return t('tray.waitingRow', { name: file.name });
  }
  if (!file.stepProgress) {
    // A file that has not reported a step yet still gets a row: an empty menu
    // while a run is definitely happening is the case this feature exists for.
    return t('tray.startingRow', { name: file.name });
  }
  const { stepName, currentStep, totalSteps } = file.stepProgress;
  return t('tray.statusRow', {
    name: file.name,
    step: tOr(stepKey(stepName), stepName),
    current: currentStep,
    total: totalSteps,
  });
};

/**
 * The rows shown above the separator, in queue order.
 *
 * Queue order is the honest order: with up to three concurrent runs there is no
 * single "current" file, and reordering by a timestamp the events only
 * approximate would make the menu jump while it is open.
 */
export const buildStatusRows = (files: VideoFile[]): string[] => {
  const rows = files
    .filter((file) => file.status === 'processing')
    .map(statusRow);

  const pending = files.filter((file) => file.status === 'pending').length;
  if (pending > 0) {
    // One row for the queue, never one per file: it answers "how much is left?"
    // without turning the menu into a second file list.
    rows.push(t('tray.queueRow', { count: pending }));
  }

  return rows.slice(0, MAX_TRAY_ROWS);
};

/**
 * The menu-bar text.
 *
 * Progress appears only while something runs: a permanent `0%` is noise in the
 * menu bar, and "idle" is legible as the absence of a number. With several files
 * in flight the value is averaged, because the menu bar has one label and two
 * runs cannot each claim it.
 */
export const buildTitle = (files: VideoFile[]): string | undefined => {
  // Only files that are actually admitted: averaging in the ones waiting for a
  // slot drags the figure toward 0 and makes a single running task look like it
  // is at half speed. If every task is waiting, there is no figure to show.
  const running = files.filter((file) => file.status === 'processing' && !file.waiting);
  if (running.length === 0) return undefined;

  const total = running.reduce((sum, file) => sum + (file.progress ?? 0), 0);
  return formatPercent(total / running.length);
};

/** What the menu should currently show, derived from the file queue alone. */
export interface TraySnapshot {
  title?: string;
  rows: string[];
}

export const traySnapshot = (files: VideoFile[]): TraySnapshot => ({
  title: buildTitle(files),
  rows: buildStatusRows(files),
});

/**
 * Push the localised labels.
 *
 * The Rust side keeps them in the same state as the progress rows and re-renders
 * the whole menu, so a language change mid-run keeps the rows.
 */
export const pushTrayLabels = async (): Promise<void> => {
  await invoke('update_tray_menu', {
    show: t('tray.show'),
    hide: t('tray.hide'),
    quit: t('tray.quit'),
  }).catch((error) => {
    // The tray is cosmetic: a failure here must never surface to the user.
    console.warn('[tray] could not update the menu labels', error);
  });
};

let timer: number | undefined;
let pendingSnapshot: TraySnapshot | undefined;
let lastKey: string | undefined;

const flush = () => {
  timer = undefined;
  const snapshot = pendingSnapshot;
  pendingSnapshot = undefined;
  if (!snapshot) return;
  void invoke('update_tray_progress', { progress: snapshot }).catch((error) => {
    console.warn('[tray] could not update the progress rows', error);
  });
};

/** Push the progress snapshot, coalescing bursts of events into one render. */
const pushTrayProgress = (files: VideoFile[]) => {
  const snapshot = traySnapshot(files);
  const key = JSON.stringify(snapshot);
  // Most `transcribe_*` events do not change what the menu shows (elapsed time
  // moves, the printed step and percentage do not).
  if (key === lastKey) return;
  lastKey = key;
  pendingSnapshot = snapshot;

  if (timer !== undefined) return;
  timer = window.setTimeout(flush, PUSH_DELAY_MS);
};

/**
 * Keep the tray mirroring the file queue.
 *
 * Subscribing to the store rather than to individual `transcribe_*` events is
 * deliberate: URL imports and "start all" change the queue without any
 * transcription event, and the tray's job is "what is the app doing", not "what
 * did the engine just say".
 */
export const startTraySync = (): (() => void) => {
  pushTrayProgress(useStore.getState().files);

  const unsubscribe = useStore.subscribe((state, previous) => {
    if (state.files !== previous.files) pushTrayProgress(state.files);
  });

  return () => {
    unsubscribe();
    if (timer !== undefined) {
      window.clearTimeout(timer);
      timer = undefined;
    }
    pendingSnapshot = undefined;
    // Forget the last push as well: React's StrictMode mounts, unmounts and
    // re-mounts this effect in development, and without this the pending push
    // cleared above would be de-duplicated away on the second mount — leaving
    // the tray showing whatever the previous run left behind.
    lastKey = undefined;
  };
};
