import { useEffect, useRef, useState } from 'react';
import { isPermissionGranted, requestPermission, sendNotification, onAction } from '@tauri-apps/plugin-notification';
import { listen } from '@tauri-apps/api/event';
import { invoke } from '@tauri-apps/api/core';
import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow';
import { resolveResource } from '@tauri-apps/api/path';
import { useTranslation } from 'react-i18next';
import i18n from './i18n';
import { ThemeProvider } from './contexts/ThemeContext';
import { Agentation } from 'agentation';
import { Header } from './components/Header';
import { Toolbar } from './components/Toolbar';
import { FileList } from './components/FileList';
import { BottomBar } from './components/BottomBar';
import { Settings } from './components/Settings';
import { useStore } from './store/useStore';
import { processQueueBatch, isModelNotInstalledError } from './lib/transcription';
import { modelNotInstalledId } from './lib/models';
import { addPathsToQueue } from './lib/fileImport';
import { reportFrontendError } from './lib/logging';
import { UrlImportDialog } from './components/UrlImportDialog';
import { ClipboardPrompt } from './components/ClipboardPrompt';
import { ensureUrlImportListeners } from './lib/urlImport';
import { useClipboardVideo } from './lib/clipboardWatch';
import { pushTrayLabels, startTraySync } from './lib/tray';

interface StepInfoPayload {
  current_step: number;
  total_steps: number;
  step_name: string;
  /** `vad` / `asr` / `diar` inside the recognition step; absent elsewhere. */
  phase?: string;
  step_elapsed: number;
  total_elapsed: number;
}

/**
 * What the translation stage did, carried on `transcribe_complete`.
 *
 * Present only when the run had a translation plan. `skipped` is a reason id, not
 * prose: the row maps it to a sentence through `translation.skip.<reason>`.
 */
interface TranslationReportPayload {
  /** `<stem>.<lang>.<ext>` paths written; empty when nothing was written. */
  written: string[];
  /** `sameLanguage` / `noSegments` when the stage decided there was nothing to do. */
  skipped?: string;
  /** Some cues kept their original text. */
  degraded: boolean;
  /** Cues that received translated text; absent on an older backend. */
  translated?: number;
  /** Cues that kept their original text; absent on an older backend. */
  kept_original?: number;
}

interface TranscribeProgressPayload {
  task_id: string;
  /**
   * `true` when the task is dispatched but blocked behind the concurrency
   * limit. It has not started work, so the row must not show a step or a
   * spinner-as-progress.
   */
  waiting?: boolean;
  /** Progress within the current step, 0…100. */
  progress: number;
  /** Progress across the whole run, 0…100; absent on non-step events. */
  overall?: number;
  status: string;
  step?: StepInfoPayload;
  total_time?: number;
  /** On `transcribe_complete` only; absent when the run never translated. */
  translation?: TranslationReportPayload;
}

let _notificationIconPath: string | undefined;
const getNotificationIconPath = async (): Promise<string | undefined> => {
  if (_notificationIconPath !== undefined) return _notificationIconPath || undefined;
  try {
    _notificationIconPath = await resolveResource('icon.png');
  } catch {
    _notificationIconPath = '';
  }
  return _notificationIconPath || undefined;
};

function App() {
  const { t } = useTranslation();
  const [isUrlImportOpen, setIsUrlImportOpen] = useState(false);
  const isSettingsOpen = useStore((state) => state.isSettingsOpen);
  // Clipboard watch: offers a one-click import when the user copies a Bilibili
  // link. Suppressed while a modal owns the screen, so the pill never fights the
  // paste dialog or the settings panel for attention.
  const { hit: clipboardHit, dismiss: dismissClipboard, accept: acceptClipboard } =
    useClipboardVideo(isUrlImportOpen || isSettingsOpen);
  const updateFile = useStore((state) => state.updateFile);
  const setIsProcessing = useStore((state) => state.setIsProcessing);
  const incrementProcessedCount = useStore((state) => state.incrementProcessedCount);
  const setCurrentTaskId = useStore((state) => state.setCurrentTaskId);

  // Use refs to avoid stale closure issues
  const filesRef = useRef(useStore.getState().files);
  const settingsRef = useRef(useStore.getState().settings);

  // Subscribe to store changes
  useEffect(() => {
    const unsubscribe = useStore.subscribe((state) => {
      filesRef.current = state.files;
      settingsRef.current = state.settings;
    });
    return unsubscribe;
  }, []);

  // Bring window to front when user clicks a system notification.
  //
  // `onAction` is mobile-only in this plugin version: it registers the
  // `actionPerformed` listener, and `tauri-plugin-notification` 2.3.3's desktop
  // `invoke_handler` only exposes `notify` / `request_permission` /
  // `is_permission_granted` — `register_listener` exists on Android and iOS only.
  // So on desktop this call always rejected with
  // "notification.registerListener not allowed. Command not found", which is
  // harmless but reached the log as an unhandled rejection on every launch (and
  // the feature it was meant to enable never worked). Guarded instead of removed,
  // so a future mobile target still gets it.
  useEffect(() => {
    if (!('__TAURI_INTERNALS__' in window)) return;
    let listener: { unregister: () => Promise<void> } | null = null;
    let disposed = false;
    onAction(() => {
      const win = getCurrentWebviewWindow();
      void win.show();
      void win.setFocus();
      void win.unminimize();
    })
      .then((fn) => {
        // The cleanup can run while this is still pending (StrictMode, unmount).
        if (disposed) {
          void fn.unregister();
          return;
        }
        listener = fn;
      })
      .catch((error) => {
        // Only a real failure is worth reporting now; the desktop "command not
        // found" case is expected and would otherwise log on every start.
        if (!String(error).includes('not allowed')) {
          reportFrontendError(error, { context: 'notification.onAction' });
        }
      });
    return () => {
      disposed = true;
      void listener?.unregister();
    };
  }, []);

  // URL import events live in their own namespace (`url_import_*`), so they are
  // attached once here and never touch the frozen `transcribe_*` contract
  // (url-import-design §5.9).
  useEffect(() => ensureUrlImportListeners(), []);

  // Mirror the queue's concurrency into the Rust semaphore.
  //
  // Two consumers, one number: the frontend decides how many tasks to dispatch,
  // and the backend decides how many may hold the GPU. Setting only one of them
  // is how the app ended up dispatching 3 while running 1. Sent on mount (so the
  // backend starts from the persisted value rather than its own default) and on
  // every change.
  const concurrency = useStore((state) => state.settings.concurrency);
  useEffect(() => {
    if (!('__TAURI_INTERNALS__' in window)) return;
    void invoke('set_transcription_concurrency', { capacity: concurrency }).catch((error) => {
      // A stale backend (an older build without the command) must not break the
      // UI: the queue still works, it just keeps the backend's default.
      reportFrontendError(error, { context: 'set_transcription_concurrency' });
    });
  }, [concurrency]);

  // Sync tray menu labels with current language. The progress rows are pushed by
  // `startTraySync` below and are kept by the Rust side, so a language change
  // never clears them (`commands/tray.rs`).
  useEffect(() => {
    void pushTrayLabels();
    const onLanguageChanged = () => void pushTrayLabels();
    i18n.on('languageChanged', onLanguageChanged);
    return () => {
      i18n.off('languageChanged', onLanguageChanged);
    };
  }, [t]);

  // Mirror the queue into the menu-bar item, which is what a left click shows.
  useEffect(() => startTraySync(), []);

  useEffect(() => {
    // Listen to transcription progress events
    const progressUnlisten = listen<TranscribeProgressPayload>('transcribe_progress', (event) => {
      const { task_id, step, waiting } = event.payload;
      console.log('[transcribe_progress]', { task_id, step, waiting });

      const updates: Partial<import('./store/useStore').VideoFile> = {
        status: 'processing',
        // A waiting task has no step to show and has not started: keeping the
        // previous `stepProgress` would leave the last run's step name on a row
        // that is only parked in the semaphore queue.
        waiting: waiting || undefined,
        // `overall`, not `progress`: the latter restarts at 0 for every step,
        // so a bar driven by it would jump backwards four times per file. The
        // tray percentage and the row's bar both read this one field.
        progress: event.payload.overall ?? event.payload.progress,
      };

      if (step) {
        // A step implies admission, so a later `waiting` event can never carry
        // one; this is belt-and-braces for an out-of-order delivery.
        updates.waiting = undefined;
        updates.stepProgress = {
          currentStep: step.current_step,
          totalSteps: step.total_steps,
          stepName: step.step_name,
          phase: step.phase,
          stepElapsed: step.step_elapsed,
          totalElapsed: step.total_elapsed,
        };
      }

      updateFile(task_id, updates);
    });

    // Listen to transcription completion events
    const completeUnlisten = listen<TranscribeProgressPayload>('transcribe_complete', (event) => {
      const { task_id, total_time, translation } = event.payload;
      console.log('[transcribe_complete]', { task_id, total_time, translation });
      updateFile(task_id, {
        status: 'completed',
        progress: 100,
        stepProgress: undefined,
        waiting: undefined,
        totalTime: total_time,
        // The run's translation outcome, so the row can say "translated to
        // English" — or why there is no translated file at all. Absent when the
        // run never asked for translation.
        translatedPaths: translation?.written.length ? translation.written : undefined,
        translationSkipped: translation?.skipped,
        translationDegraded: translation?.degraded || undefined,
        // How many cues the shortfall covers. `degraded` alone cannot tell one
        // stray line from a third of the file, and the row's wording needs the
        // difference.
        translationKeptOriginal: translation?.kept_original,
        translationTotalCues:
          translation?.translated !== undefined && translation?.kept_original !== undefined
            ? translation.translated + translation.kept_original
            : undefined,
      });
      setIsProcessing(false);
      setCurrentTaskId(null);
      incrementProcessedCount();

      // Show notification if enabled (use ref to get latest settings)
      const settings = settingsRef.current;
      const files = filesRef.current;
      if (settings.showCompletedNotification) {
        const file = files.find(f => f.id === task_id);
        void (async () => {
          let granted = await isPermissionGranted();
          if (!granted) {
            const permission = await requestPermission();
            granted = permission === 'granted';
          }
          if (granted) {
            const icon = await getNotificationIconPath();
            sendNotification({
              title: 'MonoSub',
              body: `${file?.name || 'Video'} - ${t('notification.completed')}`,
              ...(icon ? { icon } : {}),
            });
          }
        })();
      }

      if (useStore.getState().isBatchProcessing) {
        processQueueBatch();
      }
    });

    // Listen to transcription error events
    const errorUnlisten = listen<TranscribeProgressPayload>('transcribe_error', (event) => {
      const { task_id, status } = event.payload;
      console.error('[transcribe_error]', { task_id, status });
      // The row shows the failure, the log file keeps it: a release build has no
      // console to read it from later.
      reportFrontendError(status, { context: `transcribe_error ${task_id}` });
      const modelMissing = isModelNotInstalledError(status);
      // `status` is either the MODEL_NOT_INSTALLED sentinel, one of the
      // `errors.*` keys the Rust layer emits for its known failure modes, or a
      // raw message (e.g. from the subtitle writer). Resolve a key when we have
      // one and otherwise show the message as-is.
      const errorMessage =
        modelMissing || status === 'modelNotInstalled'
          ? t('errors.modelNotInstalled')
          : t(`errors.${status}`, { defaultValue: status });
      // A model that is missing (sentinel) or not in the catalogue (engine key)
      // both leave the row asking for a download. That offer lives in the row's
      // error dialog (`MissingModelPrompt`), so nothing here opens Settings: the
      // user keeps their queue and decides when to act.
      const needsModelDownload = modelMissing || status === 'modelNotInstalled';
      updateFile(task_id, {
        status: 'error',
        progress: 0,
        error: errorMessage,
        // The engine key `modelNotInstalled` is a bare key with no id attached, so
        // it resolves to `undefined` and the prompt falls back to the model list.
        missingModelId: needsModelDownload ? (modelNotInstalledId(status) ?? undefined) : undefined,
        missingModel: needsModelDownload || undefined,
        downloadProgress: undefined,
      });
      setIsProcessing(false);
      setCurrentTaskId(null);

      // Show error notification if enabled
      const settings = settingsRef.current;
      const files = filesRef.current;
      if (settings.showCompletedNotification) {
        const file = files.find(f => f.id === task_id);
        void (async () => {
          const granted = await isPermissionGranted();
          if (granted) {
            const icon = await getNotificationIconPath();
            sendNotification({
              title: 'MonoSub',
              body: `${file?.name || 'Video'} - ${errorMessage}`,
              ...(icon ? { icon } : {}),
            });
          }
        })();
      }

      if (useStore.getState().isBatchProcessing) {
        processQueueBatch();
      }
    });

    return () => {
      progressUnlisten.then((f) => f());
      completeUnlisten.then((f) => f());
      errorUnlisten.then((f) => f());
    };
  }, [updateFile, setIsProcessing, setCurrentTaskId, incrementProcessedCount, t]);

  useEffect(() => {
    let unlisten: (() => void) | null = null;
    let destroyed = false;

    const setupDropListener = async () => {
      const remove = await getCurrentWebviewWindow().onDragDropEvent((event) => {
        if (event.payload.type !== 'drop') return;
        const droppedPaths = event.payload.paths;
        if (!droppedPaths || droppedPaths.length === 0) return;

        void addPathsToQueue({
          paths: droppedPaths,
          existingFiles: filesRef.current,
          addFiles: useStore.getState().addFiles,
          source: 'WindowDrop',
        }).then((result) => {
          if (result.skippedDuplicates > 0) {
            console.warn(`[WindowDrop] Skipped ${result.skippedDuplicates} duplicate file(s)`);
          }
          if (result.skippedUnsupported > 0) {
            console.warn(`[WindowDrop] Skipped ${result.skippedUnsupported} unsupported file(s)`);
          }
        });
      });

      if (destroyed) {
        remove();
      } else {
        unlisten = remove;
      }
    };

    void setupDropListener();

    return () => {
      destroyed = true;
      if (unlisten) {
        unlisten();
      }
    };
  }, []);

  return (
    <>
      <ThemeProvider>
        <div className="h-screen overflow-hidden bg-bg-secondary dark:bg-zinc-950 flex flex-col transition-colors">
          <Header />
          <Toolbar onImportUrl={() => setIsUrlImportOpen(true)} />
          <FileList />
          <BottomBar />
          <Settings />
          <UrlImportDialog
            isOpen={isUrlImportOpen}
            onClose={() => setIsUrlImportOpen(false)}
          />
          {clipboardHit && (
            <ClipboardPrompt
              hit={clipboardHit}
              onDismiss={dismissClipboard}
              onStarted={() => acceptClipboard()}
            />
          )}
        </div>
      </ThemeProvider>
      {process.env.NODE_ENV === "development" && <Agentation />}
    </>
  );
}

export default App;
