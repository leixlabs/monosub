import { Trash2, Play, Square, CheckCircle2, AlertCircle, FolderOpen, Loader2, Clock, Users, Info, AudioLines, Cpu, FileText, Languages } from 'lucide-react';
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
import { GlassProgress } from '@/components/ui/glass-progress';
import { GlassSwitch } from '@/components/ui/glass-switch';
import { ErrorDetailsDialog } from '@/components/ErrorDetailsDialog';
import { GlassTooltip } from '@/components/ui/glass-tooltip';
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

/**
 * The icon that names what a step *does*, next to its label.
 *
 * Only the four steps the pipeline actually runs get one; the set is keyed by
 * the frozen step name (`STEP_NAMES` in `commands/transcribe.rs`), so a step
 * renamed there shows the spinner instead of silently picking up the wrong
 * icon. `Transcribing` deliberately has no icon of its own — it is the default
 * long-running case and the spinner already says so.
 */
function StepIcon({ step }: { step: string }) {
  const className = 'w-3 h-3';
  switch (step) {
    case 'Extracting audio':
      return <AudioLines className={className} aria-hidden="true" />;
    case 'Loading model':
      return <Cpu className={className} aria-hidden="true" />;
    case 'Writing subtitle':
      return <FileText className={className} aria-hidden="true" />;
    case 'Translating':
      return <Languages className={className} aria-hidden="true" />;
    // `Transcribing` intentionally falls through: it is the long-running case,
    // and the spinner beside the badge already says exactly that.
    default:
      return null;
  }
}

/**
 * The written subtitle's file name, as a button that opens it.
 *
 * A `<button>`, not a `<span onClick>`: it is an action, so it needs keyboard
 * focus and a role a screen reader can announce. The name is the base name only
 * (the row is narrow, and the directory is the same as the source in the common
 * case); the full path is the tooltip, which is also what tells the user where a
 * custom output folder put it.
 */
function SubtitleLink({ path, onOpen, label }: { path: string; onOpen: () => void; label: string }) {
  const name = path.split(/[\\/]/).pop() ?? path;
  // `GlassTooltip`, not `title=`: the app runs in a WKWebView, which does not
  // render native tooltips — the full path would be invisible.
  return (
    <GlassTooltip content={`${label}: ${path}`} side="top">
      <button
        type="button"
        onClick={onOpen}
        aria-label={`${label}: ${path}`}
        data-testid="open-subtitle"
        className="max-w-[220px] truncate text-xs text-primary hover:underline focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary/50 rounded"
      >
        {name}
      </button>
    </GlassTooltip>
  );
}

export function FileItem({ file }: FileItemProps) {
  const { t } = useTranslation();
  const removeFile = useStore((state) => state.removeFile);
  const isProcessing = useStore((state) => state.isProcessing);
  const isBatchProcessing = useStore((state) => state.isBatchProcessing);
  const settings = useStore((state) => state.settings);
  const setFileDiarization = useStore((state) => state.setFileDiarization);
  const setFileTranslate = useStore((state) => state.setFileTranslate);

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
   * Open a written subtitle with the system's default app.
   *
   * `open_file`, not `open_in_finder`: the user asked to read the subtitle, and
   * revealing it in Finder leaves them one more double-click away. The path is
   * whatever the run actually wrote (`subtitlePath` for a completed file,
   * `skippedOutputPath` for a skipped one), so a custom output folder and a
   * non-primary format both open the right file.
   *
   * A failure is logged, not shown: the row has no error slot for a file the
   * backend already reported as written, and the path is still visible in the
   * label for the user to act on themselves.
   */
  const handleOpenSubtitle = async (path: string) => {
    try {
      await invoke('open_file', { path });
    } catch (error) {
      console.error('[open_file] Failed:', error);
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

  /**
   * The label inside the badge.
   *
   * Inside the recognition step the *phase* names what is running and the step
   * name would only repeat it — "转录中 / 识别中" says 转录 twice. Everywhere else
   * there is no phase, so the step name is the only label there is.
   */
  const badgeLabel = (() => {
    const step = file.stepProgress;
    if (!step) return '';
    const key = step.stepName.toLowerCase().replace(/\s+/g, '_');
    const phase = step.phase;
    if (phase && key === 'transcribing') {
      return t(`steps.phase.${phase}`, phase);
    }
    return t(`steps.${key}`, step.stepName);
  })();

  const getStatusBadge = () => {
    switch (file.status) {
      case 'pending':
        // Two different kinds of "not started yet", and they must not read the
        // same. `isBatchProcessing` means the queue is running, so this row is
        // behind the concurrency limit and will start on its own; without it,
        // nothing is running and the row is waiting for the user to press 开始.
        // Calling the first one "等待处理" reads as "waiting for you", which is
        // the wrong instruction.
        return (
          <GlassBadge variant="default" size="sm">
            {isBatchProcessing ? t('fileItem.queued') : t('fileItem.pending')}
          </GlassBadge>
        );
      case 'processing':
        // Dispatched but parked in the backend's semaphore queue. No step has
        // begun, so there is no percentage to show and the spinner would be a
        // lie — a static "排队中" is the honest state.
        if (file.waiting) {
          return (
            <GlassBadge variant="default" size="sm" className="gap-1.5">
              <Clock className="w-3.5 h-3.5" />
              {t('fileItem.waitingForSlot')}
            </GlassBadge>
          );
        }
        return (
          <div className="flex items-center gap-3">
            {file.stepProgress ? (
              <>
                {/* One badge, two facts: what is running and how far along the
                    whole run is. The percentage is the run-wide figure — the
                    same one the bar below draws — because printing the step's
                    own fraction next to the run's was two numbers for one
                    question. The step name is dropped inside the recognition
                    step, where the phase name already says it ("转录中 / 识别中"
                    would say 转录 twice).
                    The spinner stays alongside because it is the one thing that
                    moves on its own — the percentage only arrives with an
                    event. */}
                <Loader2 className="w-3.5 h-3.5 text-primary animate-spin" />
                <GlassBadge variant="primary" size="sm" className="gap-1.5">
                  <StepIcon step={file.stepProgress.stepName} />
                  <span>{badgeLabel}</span>
                  {file.progress !== undefined && (
                    <>
                      <span aria-hidden="true" className="opacity-50">·</span>
                      <span className="tabular-nums">{Math.round(file.progress)}%</span>
                    </>
                  )}
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
          <div className="flex items-center gap-2 min-w-0 flex-wrap">
            <GlassBadge variant="success" size="sm">
              <CheckCircle2 className="w-3.5 h-3.5 mr-1" />
              {t('fileItem.completed')}
            </GlassBadge>
            {/* The subtitle itself, clickable. This is the one thing a finished
                row can offer that the row does not already show: the file is on
                disk, and opening it is what "done" invites. `open_file` (not
                reveal) because the intent is to read it. */}
            {file.subtitlePath && (
              <SubtitleLink
                path={file.subtitlePath}
                onOpen={() => void handleOpenSubtitle(file.subtitlePath!)}
                label={t('fileItem.openSubtitle')}
              />
            )}
            {/* The translated copy, when there is one. Before this the row
                listed only the original, so a successful translation was
                invisible — the user had to go looking in Finder to find out
                whether it had worked. */}
            {file.translatedPaths?.[0] && (
              <SubtitleLink
                path={file.translatedPaths[0]}
                onOpen={() => void handleOpenSubtitle(file.translatedPaths![0])}
                label={t('fileItem.openTranslated')}
              />
            )}
            {/* Why there is no translated file, when translation was on. The
                stage refuses to write a duplicate (source already equals the
                target) and writes nothing for a silent input; both are correct
                outcomes that looked identical to a bug. */}
            {!file.translatedPaths?.length && file.translationSkipped && (
              <GlassTooltip
                content={t('fileItem.translateSkippedHint', {
                  language: settings.translateTargetLanguage,
                })}
                side="top"
              >
                <span
                  className="text-xs text-text-tertiary dark:text-gray-500"
                  data-testid="translation-skipped"
                >
                  {t(`fileItem.translateSkipped.${file.translationSkipped}`, {
                    defaultValue: t('fileItem.translateSkipped.unknown'),
                    language: settings.translateTargetLanguage,
                  })}
                </span>
              </GlassTooltip>
            )}
            {/* A partial translation: the file exists, but some cues are still
                in the source language. Saying nothing would present it as a
                clean translation. */}
            {file.translationDegraded && (
              <GlassBadge variant="warning" size="sm" data-testid="translation-degraded">
                {t('fileItem.translateDegraded')}
              </GlassBadge>
            )}
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
                is already in the custom output folder, in .vtt". It is also
                clickable: the subtitle exists, so opening it is the useful
                action — and it is the only way to check it is the one you
                wanted. */}
            {file.skippedOutputPath && (
              <SubtitleLink
                path={file.skippedOutputPath}
                onOpen={() => void handleOpenSubtitle(file.skippedOutputPath!)}
                label={t('fileItem.openSubtitle')}
              />
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
                glassBadgeVariants({ variant: 'destructive', size: 'sm' }),
                'cursor-pointer hover:bg-red-500/20 dark:hover:bg-red-500/30 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-red-400/60',
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
      <div className="flex gap-3 p-3 w-full h-full">
        {/* Thumbnail. 128×72 (16:9) rather than 140×80: the row is mostly text
            and actions, and the taller preview was setting the row's height for
            a picture nobody reads at that size. */}
        <div className="w-[128px] h-[72px] rounded-lg bg-bg-secondary dark:bg-zinc-800 flex-shrink-0 overflow-hidden relative group">
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
        <div className="flex-1 flex flex-col justify-between min-w-0">
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
                  {/* Translation, per file and for the same reason: it is a
                      per-run decision. The target language and model come from
                      Settings and are not repeated here — only whether *this*
                      file gets a translated copy. */}
                  <span className="w-1 h-1 rounded-full bg-text-tertiary dark:bg-gray-600" />
                  <span className="flex items-center gap-1.5">
                    <Languages className="w-3 h-3" />
                    {t('fileItem.translate')}
                    <GlassSwitch
                      checked={file.translate}
                      onCheckedChange={(checked) => setFileTranslate(file.id, checked)}
                      aria-label={t('fileItem.translate')}
                      title={t('fileItem.translateHint')}
                      data-testid={`file-translate-${file.id}`}
                      className="scale-[0.7] origin-left"
                    />
                  </span>
                </>
              )}
            </div>
          </div>
          <div className="mt-1.5 space-y-1.5">
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
            )}
            {/* Whole-run progress. The badge above says *what* is running and
                carries the same percentage; this draws it. `file.progress` is
                the run-wide figure (App.tsx maps `overall` onto it), so it never
                jumps back to 0 on a step change. It only appears while the row
                is processing, because a finished row has nothing left to track.
                The number is not repeated underneath — one figure, one place to
                read it. */}
            {/* Not while waiting for a slot: a 0 % bar sitting still reads as
                "stuck", and there is genuinely nothing to draw — the task has
                not been admitted yet. The badge says so in words instead. */}
            {file.status === 'processing' && !file.waiting && file.progress !== undefined && (
              <div data-testid="file-progress">
                <GlassProgress
                  value={Math.min(100, Math.max(0, file.progress))}
                  aria-label={t('fileItem.progress', {
                    percent: Math.round(file.progress),
                  })}
                />
              </div>
            )}
          </div>
        </div>

        {/* Actions */}
        <div className="flex flex-col justify-center gap-2 w-[110px]">
          {file.status === 'pending' && (
            <GlassButton
              variant="primary"
              size="sm"
              onClick={() => void handleStart()}
              // Deliberately not disabled while another file runs: the app
              // supports 3 concurrent tasks (`MAX_CONCURRENT_TASKS`), and a new
              // start is queued until a slot frees. Gating the button on the
              // global `isProcessing` made every row un-startable as soon as one
              // task was running — measured: "没有办法开始，并发 3 个也是能点击开始的，
              // 开始任务之后自动排队就好了".
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
