import { useCallback, useEffect, useRef, useState, type ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import { AlertCircle, Check, Copy, FolderOpen } from 'lucide-react';
import {
  GlassDialog,
  GlassDialogClose,
  GlassDialogContent,
  GlassDialogDescription,
  GlassDialogFooter,
  GlassDialogHeader,
  GlassDialogTitle,
} from '@/components/ui/glass-dialog';
import { GlassButton } from '@/components/ui/glass-button';
import { openLogsDirectory, reportFrontendError } from '@/lib/logging';

/** What the dialog shows. Plain text only: a `errors.*` key is already resolved
 *  by the caller (`App.tsx` / `transcription.ts`), an unknown message is shown
 *  exactly as the backend produced it. */
export interface ErrorDetails {
  /** File name, for the header row. */
  name: string;
  /** Absolute source path, shown in full so it can be copied into a terminal. */
  path: string;
  /** The complete error text — never truncated, newlines preserved. */
  message: string;
}

export interface ErrorDetailsDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  /** `null` while the row has no error text (the dialog then renders nothing). */
  details: ErrorDetails | null;
  /** Slot for actions the caller owns; rendered left of the shared buttons.
   *  Reserved for F2's "missing model + download" button. */
  extraActions?: ReactNode;
}

/** How long the copy buttons keep the "copied" label. */
const COPIED_RESET_MS = 2000;

type CopyTarget = 'path' | 'message';

/**
 * Put `text` on the clipboard.
 *
 * `navigator.clipboard` is the primary path, but the WebView is not always a
 * secure context (`tauri://localhost` on macOS), where the API is missing or
 * rejects with `NotAllowedError`. The deprecated `execCommand` selection copy is
 * the only fallback available without adding the clipboard *write* permission,
 * so it is kept deliberately; it is reached only when the modern call fails.
 */
const writeClipboard = async (text: string): Promise<void> => {
  try {
    if (navigator.clipboard?.writeText) {
      await navigator.clipboard.writeText(text);
      return;
    }
  } catch {
    // Fall through to the selection copy below.
  }

  const area = document.createElement('textarea');
  area.value = text;
  area.setAttribute('readonly', '');
  area.style.position = 'fixed';
  area.style.top = '0';
  area.style.opacity = '0';
  document.body.appendChild(area);
  try {
    area.select();
    if (!document.execCommand('copy')) {
      throw new Error('document.execCommand("copy") returned false');
    }
  } finally {
    document.body.removeChild(area);
  }
};

/**
 * The full error behind a row's error badge.
 *
 * The badge can only carry a `title` tooltip, which is unreliable in WebKit and
 * cannot be selected, so anything longer than a few words was unreadable. This
 * dialog shows the message verbatim (monospace, selectable, scrollable) and
 * offers the two things a user actually does with it: copy it, or open the log
 * directory it was written to.
 */
export function ErrorDetailsDialog({ open, onOpenChange, details, extraActions }: ErrorDetailsDialogProps) {
  const { t } = useTranslation();
  const [copied, setCopied] = useState<CopyTarget | null>(null);
  /** Inline failure of a dialog action (copy / open logs). Never silent. */
  const [actionError, setActionError] = useState('');
  const copiedTimerRef = useRef<number | null>(null);

  const clearCopiedTimer = useCallback(() => {
    if (copiedTimerRef.current !== null) {
      window.clearTimeout(copiedTimerRef.current);
      copiedTimerRef.current = null;
    }
  }, []);

  useEffect(() => clearCopiedTimer, [clearCopiedTimer]);

  // Reopening for another file must not inherit the previous "copied" state or
  // its failure text.
  useEffect(() => {
    if (open) return;
    clearCopiedTimer();
    setCopied(null);
    setActionError('');
  }, [open, clearCopiedTimer]);

  const handleCopy = async (target: CopyTarget, value: string) => {
    setActionError('');
    try {
      await writeClipboard(value);
      setCopied(target);
      clearCopiedTimer();
      copiedTimerRef.current = window.setTimeout(() => {
        copiedTimerRef.current = null;
        setCopied(null);
      }, COPIED_RESET_MS);
    } catch (copyError) {
      // A failed copy must not leave the previous "copied" label standing.
      clearCopiedTimer();
      setCopied(null);
      reportFrontendError(copyError, { context: `ErrorDetailsDialog copy ${target}` });
      setActionError(t('errors.copyFailed'));
    }
  };

  const handleOpenLogs = async () => {
    setActionError('');
    try {
      await openLogsDirectory();
    } catch (openError) {
      reportFrontendError(openError, { context: 'ErrorDetailsDialog openLogsDirectory' });
      setActionError(t('errors.logsFailed'));
    }
  };

  if (!details) return null;

  return (
    <GlassDialog open={open} onOpenChange={onOpenChange}>
      <GlassDialogContent
        // The shared primitive paints a fixed dark glass surface; this dialog is
        // tall and text-heavy, so it uses the app's adaptive pair instead
        // (`UrlImportDialog` does the same) and the close glyph follows it.
        //
        // `max-h-full` (not `85vh`) because the primitive already caps the content
        // at the wrapper's height minus its gutter; the header and footer stay put
        // and only the body scrolls, so the buttons can never be cut off.
        className="glass-medium dark:glass-dark-strong max-w-xl overflow-hidden rounded-3xl"
        closeClassName="text-text-secondary hover:text-text-primary hover:bg-black/5 dark:text-white/60 dark:hover:text-white dark:hover:bg-white/10"
        data-testid="error-details-dialog"
      >
        <GlassDialogHeader className="shrink-0">
          <GlassDialogTitle className="flex items-center gap-2 text-text-primary dark:text-white">
            <AlertCircle className="w-4 h-4 text-danger" />
            {t('errors.detailsTitle')}
          </GlassDialogTitle>
          <GlassDialogDescription className="text-text-secondary dark:text-white/60">
            {t('errors.detailsHint')}
          </GlassDialogDescription>
        </GlassDialogHeader>

        <div className="mt-4 min-h-0 flex-1 space-y-4 overflow-y-auto pr-1">
          {/* File name */}
          <div className="space-y-1">
            <p className="text-xxs font-medium uppercase tracking-wide text-text-tertiary dark:text-white/40">
              {t('errors.fileLabel')}
            </p>
            <p
              className="select-text break-words text-sm font-medium text-text-primary dark:text-gray-100"
              data-testid="error-details-name"
            >
              {details.name}
            </p>
          </div>

          {/* Full path, with its own copy button: a path is the thing a user
              pastes into a terminal or a bug report. */}
          <div className="space-y-1">
            <div className="flex items-center justify-between gap-2">
              <p className="text-xxs font-medium uppercase tracking-wide text-text-tertiary dark:text-white/40">
                {t('errors.pathLabel')}
              </p>
              <GlassButton
                variant="ghost"
                size="sm"
                onClick={() => void handleCopy('path', details.path)}
                data-testid="error-details-copy-path"
              >
                {copied === 'path' ? <Check className="w-3.5 h-3.5" /> : <Copy className="w-3.5 h-3.5" />}
                {copied === 'path' ? t('errors.copied') : t('errors.copyPath')}
              </GlassButton>
            </div>
            <p
              className="select-text break-all rounded-xl border border-divider bg-black/[0.04] px-3 py-2 font-mono text-xs text-text-primary dark:border-white/10 dark:bg-black/40 dark:text-gray-200"
              data-testid="error-details-path"
            >
              {details.path}
            </p>
          </div>

          {/* The message itself: monospace, selectable, scrollable, no truncation. */}
          <div className="space-y-1">
            <div className="flex items-center justify-between gap-2">
              <p className="text-xxs font-medium uppercase tracking-wide text-text-tertiary dark:text-white/40">
                {t('errors.messageLabel')}
              </p>
              <GlassButton
                variant="ghost"
                size="sm"
                onClick={() => void handleCopy('message', details.message)}
                data-testid="error-details-copy-message"
              >
                {copied === 'message' ? <Check className="w-3.5 h-3.5" /> : <Copy className="w-3.5 h-3.5" />}
                {copied === 'message' ? t('errors.copied') : t('errors.copyError')}
              </GlassButton>
            </div>
            <p
              className="select-text max-h-[30vh] overflow-auto whitespace-pre-wrap break-words rounded-xl border border-divider bg-black/[0.04] px-3 py-2 font-mono text-xs leading-relaxed text-text-primary dark:border-white/10 dark:bg-black/40 dark:text-gray-200"
              data-testid="error-details-message"
            >
              {details.message}
            </p>
          </div>

          {actionError && (
            <p className="text-xs text-danger" data-testid="error-details-action-error">
              {actionError}
            </p>
          )}
        </div>

        <GlassDialogFooter className="mt-4 flex-wrap items-center gap-2 sm:justify-between shrink-0">
          {/* F2 puts "missing model + download" here; nothing is rendered when
              the caller has no extra actions. */}
          {extraActions ? <div className="flex flex-wrap items-center gap-2">{extraActions}</div> : null}
          <div className="flex items-center gap-2">
            <GlassButton
              variant="outline"
              size="sm"
              onClick={() => void handleOpenLogs()}
              data-testid="error-details-open-logs"
            >
              <FolderOpen className="w-4 h-4" />
              {t('errors.openLogs')}
            </GlassButton>
            <GlassDialogClose asChild>
              <GlassButton variant="ghost" size="sm">
                {t('errors.close')}
              </GlassButton>
            </GlassDialogClose>
          </div>
        </GlassDialogFooter>
      </GlassDialogContent>
    </GlassDialog>
  );
}
