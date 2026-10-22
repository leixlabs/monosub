import { Trash2, Play, Square, CheckCircle2, AlertCircle, FolderOpen, Loader2, Clock, Users, Info } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { VideoFile } from '@/store/useStore';
import { useStore } from '@/store/useStore';
import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { isModelNotInstalledError, startTranscriptionTask } from '@/lib/transcription';
import { useEffect, useState, useRef } from 'react';
import { cn } from '@/lib/utils';
import { GlassCard } from '@/components/ui/glass-card';
import { GlassButton } from '@/components/ui/glass-button';
import { GlassBadge, glassBadgeVariants } from '@/components/ui/glass-badge';
import { GlassSwitch } from '@/components/ui/glass-switch';
import { ErrorDetailsDialog } from '@/components/ErrorDetailsDialog';
import { MissingModelPrompt } from '@/components/MissingModelPrompt';
import {
  MODEL_DOWNLOAD_COMPLETE_EVENT,
  MODEL_DOWNLOAD_ERROR_EVENT,
  MODEL_DOWNLOAD_PROGRESS_EVENT,
  downloadPercent,
  type ModelDownloadCompletePayload,
  type ModelDownloadErrorPayload,
  type ModelDownloadProgressPayload,
} from '@/lib/models';

interface FileItemProps {
  file: VideoFile;
}

export function FileItem({ file }: FileItemProps) {
  const { t } = useTranslation();
  const removeFile = useStore((state) => state.removeFile);
  const isProcessing = useStore((state) => state.isProcessing);
  const settings = useStore((state) => state.settings);
  const setFileDiarization = useStore((state) => state.setFileDiarization);

  // Live elapsed time counter
  const [liveElapsed, setLiveElapsed] = useState(0);
  const lastElapsedRef = useRef(0);
  const lastUpdateTimeRef = useRef(Date.now());
  const [errorDetailsOpen, setErrorDetailsOpen] = useState(false);

  useEffect(() => {
    if (file.status !== 'processing' || !file.stepProgress) {
      return;
    }

    // Update base time when we get new elapsed from backend
    lastElapsedRef.current = file.stepProgress.totalElapsed;
    lastUpdateTimeRef.current = Date.now();
    setLiveElapsed(file.stepProgress.totalElapsed);

    // Start interval to update live counter
    const interval = setInterval(() => {
      const timeSinceUpdate = (Date.now() - lastUpdateTimeRef.current) / 1000;
      setLiveElapsed(lastElapsedRef.current + timeSinceUpdate);
    }, 100);

    return () => clearInterval(interval);
  }, [file.status, file.stepProgress?.totalElapsed]);

  const formatElapsedTime = (seconds: number) => {
    const mins = Math.floor(seconds / 60);
    const secs = Math.floor(seconds % 60);
    if (mins > 0) {
      return `${mins}m ${secs}s`;
    }
    return `${secs}s`;
  };

  const handleStart = async () => {
    await startTranscriptionTask(file.id);
  };

  const handleDelete = () => {
    removeFile(file.id);
  };

  const handleCancel = async () => {
    if (file.status !== 'processing') return;

    try {
      await invoke('cancel_transcription', { taskId: file.id });
      useStore.getState().updateFile(file.id, { status: 'pending', progress: 0 });
    } catch (error) {
      console.error('[cancel_transcription] Failed:', error);
    }
  };

  const handleOpenInFinder = async () => {
    try {
      await invoke('open_in_finder', { path: file.path });
    } catch (error) {
      console.error('[open_in_finder] Failed:', error);
    }
  };

  /**
   * Whether this row's error is a missing model, so the dialog offers a download.
   *
   * `missingModel` is the primary signal because it is the only one that can say
   * "a model is missing but the backend could not name it"; `missingModelId` then
   * adds which one. The sentinel check is a last-resort fallback for an `error`
   * string that still carries the raw `MODEL_NOT_INSTALLED:<id>` (the two entry
   * points localise it, so this is a guard, not a live branch).
   */
  const isMissingModel =
    Boolean(file.missingModel) || Boolean(file.missingModelId) || isModelNotInstalledError(file.error);

  // Mirror the missing model's transfer onto the row.
  //
  // The dialog's own subscription cannot do this job: Radix unmounts the dialog
  // content when it closes, so a user who closes the box mid-download would otherwise
  // have no indication that anything is still running. The duplication is deliberate
  // and scoped to one model id; the backend treats a repeated start/cancel as a
  // no-op, so the two subscriptions cannot fight.
  //
  // This is display only: it never starts, cancels or resets anything, and the row
  // stays `error` throughout, so a progress event can never move the queue.
  useEffect(() => {
    const modelId = file.missingModelId;
    if (!modelId || file.status !== 'error') return;

    let disposed = false;
    const registered: UnlistenFn[] = [];
    const writeProgress = (downloadProgress: VideoFile['downloadProgress']) =>
      useStore.getState().updateFile(file.id, { downloadProgress });

    const setup = async () => {
      const listeners = await Promise.all([
        listen<ModelDownloadProgressPayload>(MODEL_DOWNLOAD_PROGRESS_EVENT, (event) => {
          if (disposed || event.payload.id !== modelId) return;
          writeProgress({ downloaded: event.payload.downloaded, total: event.payload.total });
        }),
        // Both terminals clear the mirror: a percentage that stops updating would
        // keep describing a transfer that is over.
        listen<ModelDownloadCompletePayload>(MODEL_DOWNLOAD_COMPLETE_EVENT, (event) => {
          if (disposed || event.payload.id !== modelId) return;
          writeProgress(undefined);
        }),
        listen<ModelDownloadErrorPayload>(MODEL_DOWNLOAD_ERROR_EVENT, (event) => {
          if (disposed || event.payload.id !== modelId) return;
          writeProgress(undefined);
        }),
      ]);

      if (disposed) {
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
  }, [file.id, file.missingModelId, file.status]);

  /**
   * Leave the error state without starting anything.
   *
   * The row goes back to `pending` so the Start button returns and the stale error
   * text is gone. Nothing here calls `processQueueBatch()`, and nothing needs to:
   * that loop is only entered from an explicit "start all", from a `transcribe_*`
   * event, or from a URL import's auto-start — there is no store subscription that
   * reacts to a new `pending` row, so this reset starts nothing by itself.
   *
   * The row's status is the dialog's lifetime. Until this runs the row stays
   * `error`, which is what keeps `MissingModelPrompt` mounted through a finished
   * download; once it runs the `error` branch unmounts and the dialog goes with it.
   * `errorDetailsOpen` therefore needs no closing here — its owner is being
   * unmounted, and calling setState on it would be the actual bug.
   *
   * The one case where `pending` still means "will run": a batch covering *other*
   * files is in flight, and the loop's next re-entry picks this row up like any
   * other pending file. That is the queue's existing meaning of pending
   * (`cancel_transcription` resets rows the same way), so it is documented rather
   * than special-cased. A batch whose *only* remaining file was this one has already
   * stopped before the user could click: `transcribe_error` re-enters
   * `processQueueBatch`, finds no pending and no processing files, and clears
   * `isBatchProcessing`.
   */
  const resetFileAfterModelDownload = () => {
    useStore.getState().updateFile(file.id, {
      status: 'pending',
      progress: 0,
      error: undefined,
      missingModelId: undefined,
      missingModel: undefined,
      downloadProgress: undefined,
    });
  };

  const handleModelDownloadedStart = () => {
    // `startTranscriptionTask` clears the same fields; doing it first keeps the row
    // from flashing an error badge while its own reset is still in flight.
    resetFileAfterModelDownload();
    // `errorDetailsOpen` is this component's state, and `FileItem` itself stays
    // mounted (its key is the file id) — only the `error` branch unmounts. The write
    // is therefore safe, and it matters: leaving it `true` would make the next error
    // on this file open its dialog unbidden.
    setErrorDetailsOpen(false);
    // Scheduled directly, never through `processQueueBatch`: this is one file the
    // user just asked to run, not a batch.
    void startTranscriptionTask(file.id);
  };

  const handleModelDownloadedLater = () => {
    resetFileAfterModelDownload();
    setErrorDetailsOpen(false);
  };

  const formatFileSize = (bytes: number) => {
    if (bytes <= 0) return '--';
    if (bytes < 1024) return bytes + ' B';
    if (bytes < 1024 * 1024) return (bytes / 1024).toFixed(1) + ' KB';
    if (bytes < 1024 * 1024 * 1024) return (bytes / (1024 * 1024)).toFixed(1) + ' MB';
    return (bytes / (1024 * 1024 * 1024)).toFixed(1) + ' GB';
  };

  const formatDuration = (seconds: number) => {
    if (seconds <= 0) return '--:--';
    const hours = Math.floor(seconds / 3600);
    const minutes = Math.floor((seconds % 3600) / 60);
    const secs = Math.floor(seconds % 60);

    if (hours > 0) {
      return `${hours}:${minutes.toString().padStart(2, '0')}:${secs.toString().padStart(2, '0')}`;
    }
    return `${minutes}:${secs.toString().padStart(2, '0')}`;
  };

  const getStatusBadge = () => {
    switch (file.status) {
      case 'pending':
        return (
          <GlassBadge variant="default" size="sm">
            {t('fileItem.pending')}
          </GlassBadge>
        );
      case 'processing':
        return (
          <div className="flex items-center gap-3">
            {file.stepProgress ? (
              <>
                {/* Step indicator */}
                <div
                  className="flex items-center gap-2"
                  // A bare "3/5" reads as a percentage, a retry count or a file
                  // counter. The visible text names what it counts, and the
                  // tooltip adds the step heading it belongs to.
                  title={t('fileItem.stepOfTotalHint', {
                    current: file.stepProgress.currentStep,
                    total: file.stepProgress.totalSteps,
                    step: t(`steps.${file.stepProgress.stepName.toLowerCase().replace(/\s+/g, '_')}`, file.stepProgress.stepName),
                  })}
                >
                  <Loader2 className="w-3.5 h-3.5 text-primary animate-spin" />
                  <span className="text-xs font-medium text-text-secondary dark:text-gray-400 whitespace-nowrap">
                    {t('fileItem.stepOfTotal', {
                      current: file.stepProgress.currentStep,
                      total: file.stepProgress.totalSteps,
                    })}
                  </span>
                </div>
                {/* Step name */}
                <GlassBadge variant="primary" size="sm">
                  {t(`steps.${file.stepProgress.stepName.toLowerCase().replace(/\s+/g, '_')}`, file.stepProgress.stepName)}
                </GlassBadge>
                {/* Elapsed time */}
                <div className="flex items-center gap-1 text-xs text-text-tertiary dark:text-gray-500">
                  <Clock className="w-3 h-3" />
                  <span>{formatElapsedTime(liveElapsed)}</span>
                </div>
              </>
            ) : (
              <div className="flex items-center gap-2">
                <Loader2 className="w-3.5 h-3.5 text-primary animate-spin" />
                <GlassBadge variant="primary" size="sm">{t('fileItem.starting')}</GlassBadge>
              </div>
            )}
          </div>
        );
      case 'completed':
        return (
          <div className="flex items-center gap-2">
            <GlassBadge variant="success" size="sm">
              <CheckCircle2 className="w-3.5 h-3.5 mr-1" />
              {t('fileItem.completed')}
            </GlassBadge>
            {file.totalTime !== undefined && (
              <span className="text-xs text-text-tertiary dark:text-gray-500">
                {formatElapsedTime(file.totalTime)}
              </span>
            )}
          </div>
        );
      case 'skipped':
        return (
          <div className="flex items-center gap-2 min-w-0">
            <GlassBadge variant="default" size="sm">
              <CheckCircle2 className="w-3.5 h-3.5 mr-1" />
              {t('fileItem.skipped')}
            </GlassBadge>
            {/* Which file caused the skip. Without it "已跳过" reads as
                "nothing to do here", when the real answer may be "your subtitle
                is already in the custom output folder, in .vtt". */}
            {file.skippedOutputPath && (
              <span
                className="text-xs text-text-tertiary dark:text-gray-500 truncate"
                title={file.skippedOutputPath}
              >
                {file.skippedOutputPath.split(/[\\/]/).pop()}
              </span>
            )}
          </div>
        );
      case 'error':
        // A real <button>, not a div: the full text only fits in a dialog, and a
        // `title` tooltip is both unreliable in WebKit and impossible to select.
        return (
          <>
            <button
              type="button"
              onClick={() => setErrorDetailsOpen(true)}
              aria-label={t('fileItem.showErrorDetails', { name: file.name })}
              title={file.error || t('fileItem.error')}
              data-testid="file-error-badge"
              className={cn(
                // No `size`: `GlassBadge` forwards only `variant` to the cva call,
                // so the row's other badges render at its `md` default. Matching
                // that keeps this badge the same size it has always been.
                glassBadgeVariants({ variant: 'destructive' }),
                'cursor-pointer hover:bg-red-500/30 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-red-400/60',
              )}
            >
              <AlertCircle className="w-3.5 h-3.5 mr-1" />
              {t('fileItem.error')}
            </button>
            <ErrorDetailsDialog
              open={errorDetailsOpen}
              onOpenChange={setErrorDetailsOpen}
              details={{
                name: file.name,
                path: file.path,
                // Fall back to the badge's own word rather than opening an
                // empty box: the dialog must always have something to show.
                message: file.error || t('fileItem.errorUnknown'),
              }}
              // Only a missing model gets the download panel. A finished download
              // does not close the dialog or reset the row: the user decides
              // whether to run the file (see `MissingModelPrompt`).
              extraActions={
                isMissingModel ? (
                  <MissingModelPrompt
                    modelId={file.missingModelId ?? null}
                    onStart={handleModelDownloadedStart}
                    startDisabled={isProcessing}
                    activeDownload={file.downloadProgress ?? null}
                    onDismiss={handleModelDownloadedLater}
                    onProgressCleared={() =>
                      useStore.getState().updateFile(file.id, { downloadProgress: undefined })
                    }
                  />
                ) : undefined
              }
            />
          </>
        );
      default:
        return null;
    }
  };

  return (
    <GlassCard
      glowEffect={file.status === 'processing'}
      className={`animate-fade-in ${file.status === 'processing' ? '' : 'hover:-translate-y-1 transition-transform duration-200'
        }`}
    >
      <div className="flex gap-4 p-4 w-full h-full">
        {/* Thumbnail */}
        <div className="w-[140px] h-[80px] rounded-lg bg-bg-secondary dark:bg-zinc-800 flex-shrink-0 overflow-hidden relative group">
          {file.thumbnail ? (
            <img
              src={file.thumbnail}
              alt={file.name}
              className="w-full h-full object-cover"
            />
          ) : (
            <div className="w-full h-full flex items-center justify-center text-text-tertiary dark:text-gray-600">
              <Play className="w-8 h-8" />
            </div>
          )}
          {/* Duration overlay */}
          <div className="absolute bottom-1.5 right-1.5 px-1.5 py-0.5 bg-black/70 dark:bg-black/80 rounded text-[10px] text-white font-medium">
            {formatDuration(file.duration)}
          </div>
        </div>

        {/* Details */}
        <div className="flex-1 flex flex-col justify-between min-w-0 py-0.5">
          <div>
            <h3 className="text-sm font-medium text-text-primary dark:text-white truncate mb-1">
              {file.name}
            </h3>
            <div className="flex items-center gap-3 text-xs text-text-tertiary dark:text-gray-500">
              <span>{formatFileSize(file.size)}</span>
              <span className="w-1 h-1 rounded-full bg-text-tertiary dark:bg-gray-600" />
              <span>{settings.outputFormat.toUpperCase()}</span>
              {/* Per-file speaker separation. Editable only while the run has
                  not started: changing it mid-flight would not affect the
                  request already sent to the engine. */}
              {file.status === 'pending' && (
                <>
                  <span className="w-1 h-1 rounded-full bg-text-tertiary dark:bg-gray-600" />
                  <span className="flex items-center gap-1.5">
                    <Users className="w-3 h-3" />
                    {t('fileItem.multiSpeaker')}
                    <GlassSwitch
                      checked={file.diarization}
                      onCheckedChange={(checked) => setFileDiarization(file.id, checked)}
                      aria-label={t('fileItem.multiSpeaker')}
                      title={t('fileItem.multiSpeakerHint')}
                      className="scale-[0.7] origin-left"
                    />
                  </span>
                </>
              )}
            </div>
          </div>
          <div className="mt-2 space-y-1.5">
            {getStatusBadge()}
            {/* URL imports only: the platform had a subtitle behind a login, so
                this file is being recognised only because cookies are not set up
                (url-import-design §4.5). A hint, never an error. */}
            {file.loginHint && file.status !== 'completed' && (
              <p
                className="flex items-start gap-1.5 text-xs text-text-tertiary dark:text-gray-500"
                data-testid="file-login-hint"
              >
                <Info className="w-3 h-3 mt-0.5 shrink-0" />
                {t('urlImport.login.queuedHint')}
              </p>
            )}
            {/* The error dialog owns the download controls; this is the same
                transfer seen from the queue, so closing the dialog does not hide
                that something is still running. */}
            {file.status === 'error' && file.downloadProgress && (
              <p
                className="text-xs text-text-tertiary dark:text-gray-500"
                data-testid="file-model-download-progress"
              >
                {t('errors.missingModel.downloading', {
                  percent: downloadPercent(file.downloadProgress.downloaded, file.downloadProgress.total),
                })}
              </p>
            )}          </div>
        </div>

        {/* Actions */}
        <div className="flex flex-col justify-center gap-2 w-[110px]">
          {file.status === 'pending' && (
            <GlassButton
              variant="primary"
              size="sm"
              onClick={() => void handleStart()}
              disabled={isProcessing}
              className="w-full"
            >
              <Play className="w-4 h-4" />
              {t('fileItem.start')}
            </GlassButton>
          )}
          {file.status === 'processing' && (
            <GlassButton
              variant="destructive"
              size="sm"
              onClick={() => void handleCancel()}
              className="w-full"
            >
              <Square className="w-4 h-4" />
              {t('fileItem.cancel')}
            </GlassButton>
          )}
          {(file.status === 'completed' || file.status === 'skipped') && (
            <GlassButton
              variant="default"
              size="sm"
              onClick={() => void handleOpenInFinder()}
              className="w-full text-success border-success/30 hover:bg-success/20"
            >
              <FolderOpen className="w-4 h-4" />
              {t('fileItem.open')}
            </GlassButton>
          )}
          <GlassButton
            variant="ghost"
            size="sm"
            onClick={handleDelete}
            className="w-full text-text-tertiary dark:text-gray-500 hover:text-danger"
          >
            <Trash2 className="w-4 h-4" />
            {t('fileItem.delete')}
          </GlassButton>
        </div>
      </div>
    </GlassCard>
  );
}
