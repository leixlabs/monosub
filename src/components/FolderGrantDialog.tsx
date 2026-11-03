import { useSyncExternalStore } from 'react';
import { useTranslation } from 'react-i18next';
import { FolderLock } from 'lucide-react';
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
import {
  answerFolderGrant,
  getFolderGrantRequest,
  subscribeFolderGrantRequest,
} from '@/lib/grants';

/**
 * The explanation that precedes the system folder panel.
 *
 * Under the sandbox, "save the subtitle next to the video" cannot work until the
 * user has authorized that folder — picking a *file* does not grant its
 * directory. Springing the panel without a word reads as "why am I choosing a
 * folder again?", so this says what is about to happen and why, and the panel
 * opens only after the user agrees.
 *
 * The request is owned by `lib/grants.ts` rather than the store: its callers are
 * plain functions (the batch start and the per-file start), and they await an
 * answer. This component only renders that pending request.
 */
export function FolderGrantDialog() {
  const { t } = useTranslation();
  const request = useSyncExternalStore(
    subscribeFolderGrantRequest,
    getFolderGrantRequest,
    getFolderGrantRequest,
  );

  const dirs = request?.dirs ?? [];
  // Long queues can span many folders; listing a dozen paths buries the point.
  const shown = dirs.slice(0, 3);
  const extra = dirs.length - shown.length;

  return (
    <GlassDialog
      open={request !== null}
      // Escape must answer, not just hide: the caller is awaiting a boolean and
      // would otherwise never dispatch the queue it already accepted.
      onOpenChange={(open) => {
        if (!open) answerFolderGrant(false);
      }}
    >
      <GlassDialogContent
        className="glass-medium dark:glass-dark-strong max-w-md rounded-3xl"
        closeClassName="text-text-secondary hover:text-text-primary hover:bg-black/5 dark:text-white/60 dark:hover:text-white dark:hover:bg-white/10"
        data-testid="folder-grant-dialog"
      >
        <GlassDialogHeader>
          <GlassDialogTitle className="flex items-center gap-2 text-text-primary dark:text-white">
            <FolderLock className="w-4 h-4 text-accent" />
            {t('grants.title')}
          </GlassDialogTitle>
          <GlassDialogDescription className="text-text-secondary dark:text-white/60">
            {t('grants.body')}
          </GlassDialogDescription>
        </GlassDialogHeader>

        <div className="mt-4 space-y-2">
          {shown.map((dir) => (
            <p
              key={dir}
              className="select-text break-all rounded-xl border border-divider bg-black/[0.04] px-3 py-2 font-mono text-xs text-text-primary dark:border-white/10 dark:bg-black/40 dark:text-gray-200"
              data-testid="folder-grant-dir"
            >
              {dir}
            </p>
          ))}
          {extra > 0 && (
            <p className="text-xs text-text-tertiary dark:text-white/40">
              {t('grants.moreFolders', { count: extra })}
            </p>
          )}
          <p className="text-xs leading-relaxed text-text-tertiary dark:text-white/50">
            {t('grants.hint')}
          </p>
        </div>

        <GlassDialogFooter className="mt-4">
          <GlassDialogClose asChild>
            <GlassButton variant="ghost" size="sm" data-testid="folder-grant-cancel">
              {t('grants.cancel')}
            </GlassButton>
          </GlassDialogClose>
          <GlassButton
            variant="primary"
            size="sm"
            onClick={() => answerFolderGrant(true)}
            data-testid="folder-grant-allow"
          >
            {t('grants.allow')}
          </GlassButton>
        </GlassDialogFooter>
      </GlassDialogContent>
    </GlassDialog>
  );
}
