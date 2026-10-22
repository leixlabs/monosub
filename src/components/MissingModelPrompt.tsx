import { useCallback, useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { Download, Loader2, Play, Settings2, X } from 'lucide-react';
import { GlassButton } from '@/components/ui/glass-button';
import { reportFrontendError } from '@/lib/logging';
import {
  MODEL_DOWNLOAD_COMPLETE_EVENT,
  MODEL_DOWNLOAD_ERROR_EVENT,
  MODEL_DOWNLOAD_PROGRESS_EVENT,
  cancelModelDownload,
  downloadModel,
  downloadPercent,
  formatModelSize,
  isTranslationModel,
  listModels,
  modelManifestSize,
  openModelsSettings,
  type ModelDownloadCompletePayload,
  type ModelDownloadErrorPayload,
  type ModelDownloadProgressPayload,
  type ModelStatus,
} from '@/lib/models';

/** Bytes transferred so far, as `model_download_progress` reports them. */
interface ModelDownloadBytes {
  downloaded: number;
  total: number;
}

export interface MissingModelPromptProps {
  /** Catalogue id parsed out of `MODEL_NOT_INSTALLED:<id>`; `null` when the
   *  sentinel carried no id, in which case the prompt can only point at the list. */
  modelId: string | null;
  /**
   * Run the failed file again, now that its model is installed.
   *
   * Owned by the caller (`FileItem`), not by this component: starting a task also
   * resets the row out of its error state and closes the dialog, neither of which
   * belongs to a prompt about a download.
   */
  onStart?: () => void;
  /**
   * Disables "Start transcription" while the app is already running something.
   *
   * The same rule the pending row's own Start button follows (`FileItem`), so a
   * manual click cannot push past what "start all" is willing to schedule.
   */
  startDisabled?: boolean;
  /**
   * The transfer the row is already mirroring, used when the dialog is reopened
   * mid-download.
   *
   * The backend download outlives this component: closing the dialog unmounts the
   * listeners, so without this the reopened panel would show "Download" for a
   * transfer that is already running. `FileItem` keeps the mirror on the row for
   * exactly that reason and hands it back here.
   */
  activeDownload?: { downloaded: number; total: number } | null;
  /**
   * "Later" — the model is installed but the user does not want to run the file
   * now. The caller resets the row to `pending` without starting anything.
   *
   * Without this escape hatch the row would keep an error that stopped being true
   * the moment the download finished.
   */
  onDismiss: () => void;
  /**
   * The row's mirrored progress must be cleared.
   *
   * A cancelled transfer emits no terminal event, so nothing else would ever
   * clear `downloadProgress` and the queue row would keep reporting the last
   * percentage of a download that is no longer running (measured in the running
   * app). The caller owns that mirror, so it clears it here.
   */
  onProgressCleared?: () => void;
}

/**
 * What a `MODEL_NOT_INSTALLED:<id>` failure can do for the user: name the missing
 * model, download it in place, then hand the decision to start back to them.
 *
 * Rendered into `ErrorDetailsDialog`'s `extraActions` slot, so it never owns a
 * modal of its own — the dialog the user opened to read the error is also where
 * the fix lives. (A missing model used to throw the user into Settings ▸ Models,
 * losing the file they were working on.)
 *
 * Three rules this component exists to honour:
 *
 * * **The catalogue, not the event, decides "installed".** `isInstalled` below is
 *   derived from `model.installed`, which comes from `list_models()`. That is what
 *   makes a *reload* work: `missingModelId` can outlive the download (the dialog
 *   was closed when it finished, or the app was restarted), and reopening the
 *   badge must then show "Start transcription" rather than a Download button whose
 *   click would necessarily fail. The `model_download_complete` handler is only an
 *   accelerator: optimistic flip on the same tick, then one `list_models()` to
 *   confirm — `is_installed` also checks file sizes, so the event and the
 *   catalogue can disagree for a moment and the catalogue wins.
 * * **Downloading never starts a transcription.** Nothing in this file calls
 *   `processQueueBatch()` or `startTranscriptionTask()`: the completion handler
 *   leaves the row in `error`, and the only start is `onStart`, from a click.
 * * **A failed download keeps the dialog open** and shows the message, because
 *   the run is still blocked and closing the box would hide the only explanation.
 *
 * Lifecycle note: the dialog outlives the download but not the row's `error`
 * status. A click on "Start transcription" / "Later" resets the row in `FileItem`,
 * the `error` branch unmounts, and this component goes with it — which is why the
 * reset does not try to close a dialog or clean up its own state from outside.
 */
export function MissingModelPrompt({
  modelId,
  onStart,
  startDisabled = false,
  activeDownload = null,
  onDismiss,
  onProgressCleared,
}: MissingModelPromptProps) {
  const { t } = useTranslation();
  const [model, setModel] = useState<ModelStatus | null>(null);
  const [isLoadingModel, setIsLoadingModel] = useState(true);
  const [isDownloading, setIsDownloading] = useState(false);
  const [bytes, setBytes] = useState<ModelDownloadBytes | null>(null);
  /** A failure of this panel's own action; shown in place, never swallowed. */
  const [actionError, setActionError] = useState('');

  /**
   * Bumped by every lookup and by unmount, so a slow `list_models` that resolves
   * after the dialog moved to another row (or closed) cannot overwrite fresher
   * state.
   */
  const requestSeqRef = useRef(0);

  /**
   * Set synchronously when `download_model` is called, so a double click cannot
   * register the same transfer twice before React state settles.
   *
   * Rust would ignore the second start ("Download already in progress"), but the
   * panel would then have two callers racing to own the in-flight state.
   */
  const startGuardRef = useRef(false);

  // Read inside the reset effect only: making it a dependency would re-run the
  // whole per-model reset on every progress tick.
  const activeDownloadRef = useRef(activeDownload);
  activeDownloadRef.current = activeDownload;

  /** Re-read this model's catalogue entry; the catalogue is the install truth. */
  const loadModel = useCallback(async () => {
    const seq = ++requestSeqRef.current;
    if (!modelId) {
      setModel(null);
      setIsLoadingModel(false);
      return;
    }
    try {
      const models = await listModels();
      if (seq !== requestSeqRef.current) return;
      setModel(models.find((candidate) => candidate.id === modelId) ?? null);
    } catch (error) {
      if (seq !== requestSeqRef.current) return;
      // An unreadable catalogue must not hide the dialog: fall back to the
      // "we cannot name it" answer, which still offers the model list.
      console.warn('[MissingModelPrompt] could not read the model catalogue', error);
      reportFrontendError(error, { context: `MissingModelPrompt list_models ${modelId}` });
      setModel(null);
    } finally {
      if (seq === requestSeqRef.current) setIsLoadingModel(false);
    }
  }, [modelId]);

  // Reset per model: reopening the dialog for another row must not inherit the
  // previous transfer's percentage or failure text.
  useEffect(() => {
    // A transfer the row was already mirroring is the one exception: the download
    // keeps running while the dialog is closed, so the reopened panel resumes from
    // it instead of offering a second start. The mirror only counts while the
    // model is still absent, though — `loadModel()` below settles that, and a
    // stale percentage for an installed model is dropped there.
    const active = activeDownloadRef.current;
    setModel(null);
    setIsLoadingModel(true);
    setIsDownloading(Boolean(active));
    setBytes(active ?? null);
    setActionError('');
    startGuardRef.current = false;
    void loadModel();

    return () => {
      requestSeqRef.current += 1;
    };
  }, [loadModel]);

  // A model that is already installed cannot have a transfer in flight, so a
  // mirror restored from the row is stale — clear it instead of showing
  // "downloading… 38%" next to "Start transcription".
  //
  // Guarded by a ref because `onProgressCleared` writes the store, which re-renders
  // `FileItem` and re-runs this effect: without the guard React reports
  // "Maximum update depth" / nested update (measured — it blanked the window). One
  // clear per mount is enough, since the mirror can only be re-seeded by reopening
  // the dialog (a fresh mount).
  const clearedStaleProgressRef = useRef(false);
  useEffect(() => {
    if (!model?.installed) return;
    if (clearedStaleProgressRef.current) return;
    if (!isDownloading && !bytes) return;
    clearedStaleProgressRef.current = true;
    setIsDownloading(false);
    setBytes(null);
    onProgressCleared?.();
  }, [model?.installed, isDownloading, bytes, onProgressCleared]);

  // The three download events. Every handler ignores other ids: a download is
  // global, and `Settings.tsx` subscribes to the same three events for its rows.
  //
  // StrictMode mounts this effect twice in development. `disposed` makes the
  // first (discarded) mount inert, and unlisteners that resolve *after* that
  // cleanup has run unsubscribe themselves — otherwise the discarded mount would
  // keep a live listener whose `setState` fires into an unmounted tree.
  useEffect(() => {
    if (!modelId) return;

    let disposed = false;
    const registered: UnlistenFn[] = [];

    const setup = async () => {
      const listeners = await Promise.all([
        listen<ModelDownloadProgressPayload>(MODEL_DOWNLOAD_PROGRESS_EVENT, (event) => {
          if (disposed || event.payload.id !== modelId) return;
          const next = { downloaded: event.payload.downloaded, total: event.payload.total };
          setBytes(next);
          // Progress is the honest signal for "a transfer is running", whoever
          // started it: this panel may have been reopened mid-download, or the
          // download may have been started from Settings, and the backend task is
          // global either way. So the button follows the events, not the click.
          setIsDownloading(true);
        }),
        listen<ModelDownloadCompletePayload>(MODEL_DOWNLOAD_COMPLETE_EVENT, (event) => {
          if (disposed || event.payload.id !== modelId) return;
          setIsDownloading(false);
          setBytes(null);
          // Optimistic flip so the panel answers on this tick, then confirm against
          // the catalogue — `is_installed` also compares file sizes, so the event and
          // `list_models` can disagree for a moment, and the catalogue is the
          // authority. Neither branch starts a transcription.
          setModel((current) => (
            current ? { ...current, installed: true, size_on_disk: modelManifestSize(current) } : current
          ));
          void loadModel();
        }),
        listen<ModelDownloadErrorPayload>(MODEL_DOWNLOAD_ERROR_EVENT, (event) => {
          if (disposed || event.payload.id !== modelId) return;
          setIsDownloading(false);
          setBytes(null);
          // A failed transfer is the kind of failure a user reports later, when
          // the banner is gone — so it goes to the log as well as the panel.
          reportFrontendError(event.payload.message, { context: `model_download_error ${event.payload.id}` });
          setActionError(t('errors.missingModel.failed', { message: event.payload.message }));
        }),
      ]);

      if (disposed) {
        // The cleanup ran while these were being registered.
        listeners.forEach((unlisten) => unlisten());
        return;
      }
      registered.push(...listeners);
    };

    void setup();

    return () => {
      disposed = true;
      registered.forEach((unlisten) => unlisten());
    };
  }, [modelId, loadModel, t]);

  const handleDownload = async () => {
    if (!modelId || startGuardRef.current) return;
    startGuardRef.current = true;
    setActionError('');
    setIsDownloading(true);
    setBytes({ downloaded: 0, total: 0 });
    try {
      // `download_model` returns as soon as the BACKGROUND task is spawned, not
      // when the transfer finishes, so the in-flight state must survive this
      // resolve: it is cleared by the complete/error events, and by the catch
      // below when the request itself is rejected (then no events will arrive).
      await downloadModel(modelId);
    } catch (error) {
      reportFrontendError(error, { context: `download_model ${modelId}` });
      setActionError(t('errors.missingModel.failed', { message: String(error) }));
      setIsDownloading(false);
      setBytes(null);
    } finally {
      startGuardRef.current = false;
    }
  };

  /**
   * Cancel the transfer *and* clear the row's mirror of it.
   *
   * `cancelModelDownload` reports its own outcome and emits no
   * `model_download_error`, so nothing else would ever clear `downloadProgress`:
   * the queue row kept saying "downloading… 38%" forever after a cancel
   * (measured in the running app). Only the local `bytes` reset was done here,
   * which fixed the dialog and left the row stale.
   */
  const handleCancelDownload = async () => {
    if (!modelId) return;
    startGuardRef.current = false;
    setActionError('');
    try {
      // A cancelled transfer emits no error event (the cancel command reports its
      // own outcome), so this local reset is what flips the button back.
      await cancelModelDownload(modelId);
    } catch (error) {
      reportFrontendError(error, { context: `cancel_model_download ${modelId}` });
      setActionError(t('errors.missingModel.failed', { message: String(error) }));
    }
    setIsDownloading(false);
    setBytes(null);
    onProgressCleared?.();
  };

  // No id, or an id the catalogue does not know: name the limit instead of
  // guessing, and point at the one place that lists every model.
  if (!modelId || (!isLoadingModel && !model)) {
    return (
      <div className="flex flex-wrap items-center gap-2" data-testid="missing-model-prompt">
        <p className="text-xs text-text-secondary dark:text-white/70">
          {t('errors.missingModel.unknown')}
        </p>
        <GlassButton
          variant="outline"
          size="sm"
          onClick={openModelsSettings}
          data-testid="missing-model-settings"
        >
          <Settings2 className="w-4 h-4" />
          {t('errors.missingModel.openSettings')}
        </GlassButton>
      </div>
    );
  }

  if (!model) {
    return (
      <p
        className="flex items-center gap-2 text-xs text-text-secondary dark:text-white/70"
        data-testid="missing-model-prompt"
      >
        <Loader2 className="w-3.5 h-3.5 animate-spin" />
        {t('errors.missingModel.lookingUp')}
      </p>
    );
  }

  const percent = bytes ? downloadPercent(bytes.downloaded, bytes.total) : 0;

  // The single source of truth for the panel's two faces. Derived from `model`,
  // which `loadModel()` reads from `list_models()` — never from an event — so a
  // dialog reopened after a reload shows "Start transcription" for a model that is
  // already on disk.
  const isInstalled = model.installed;

  return (
    <div
      className="space-y-2 rounded-xl border border-divider bg-bg-secondary/70 px-3 py-2 dark:border-white/10 dark:bg-white/5"
      data-testid="missing-model-prompt"
    >
      <p className="text-xs text-text-primary dark:text-gray-100" data-testid="missing-model-name">
        <span className="font-semibold">{model.display_name || model.id}</span>
        <span className="text-text-secondary dark:text-white/60">
          {' · '}
          {isTranslationModel(model)
            ? t('errors.missingModel.roleTranslation')
            : t('errors.missingModel.roleRecognition')}
          {' · '}
          {t('models.size')}: {formatModelSize(modelManifestSize(model))}
        </span>
      </p>

      <p className="text-xs text-text-secondary dark:text-white/70" data-testid="missing-model-hint">
        {isInstalled ? t('errors.missingModel.installed') : t('errors.missingModel.needsDownload')}
      </p>

      {/* Progress is only meaningful while a transfer can still be running, so it
          is gated on `!isInstalled` too. `bytes` can outlive the download that
          produced it — the row's mirror is restored from `activeDownload` when the
          dialog is reopened — and an "installed" model showing
          "downloading… 38%" is a self-contradiction the user cannot act on
          (measured in the running app). */}
      {isDownloading && !isInstalled && (
        <div className="space-y-1">
          <div className="h-1.5 w-full overflow-hidden rounded-full bg-divider dark:bg-zinc-700">
            <div
              className="h-full rounded-full bg-primary transition-all duration-300"
              style={{ width: `${percent}%` }}
            />
          </div>
          <p className="text-xxs text-text-secondary dark:text-white/60" data-testid="missing-model-progress">
            {t('errors.missingModel.downloading', { percent })}
          </p>
        </div>
      )}

      {actionError && (
        <p className="text-xs text-danger" data-testid="missing-model-error">
          {actionError}
        </p>
      )}

      <div className="flex flex-wrap items-center gap-2">
        {isInstalled ? (
          <>
            {/* The start button is opt-in: a finished download never runs the
                file on its own, and the row is still `error` at this point. */}
            {onStart && (
              <GlassButton
                variant="primary"
                size="sm"
                onClick={onStart}
                disabled={startDisabled}
                data-testid="missing-model-start"
              >
                <Play className="w-4 h-4" />
                {t('errors.missingModel.start')}
              </GlassButton>
            )}
            <GlassButton
              variant="ghost"
              size="sm"
              onClick={onDismiss}
              data-testid="missing-model-later"
            >
              {t('errors.missingModel.later')}
            </GlassButton>
          </>
        ) : (
          <>
            {isDownloading ? (
              <GlassButton
                variant="outline"
                size="sm"
                onClick={() => void handleCancelDownload()}
                data-testid="missing-model-cancel"
              >
                <X className="w-4 h-4" />
                {t('errors.missingModel.cancelDownload')}
              </GlassButton>
            ) : (
              <GlassButton
                variant="primary"
                size="sm"
                onClick={() => void handleDownload()}
                data-testid="missing-model-download"
              >
                <Download className="w-4 h-4" />
                {t('errors.missingModel.download')}
              </GlassButton>
            )}
            {/* Only an explicit click moves the user to the model list; the error
                path itself no longer redirects them there. */}
            <GlassButton
              variant="outline"
              size="sm"
              onClick={openModelsSettings}
              data-testid="missing-model-settings"
            >
              <Settings2 className="w-4 h-4" />
              {t('errors.missingModel.openSettings')}
            </GlassButton>
          </>
        )}
      </div>
    </div>
  );
}
