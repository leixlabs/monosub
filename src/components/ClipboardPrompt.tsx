import { useTranslation } from 'react-i18next';
import { Link2, Loader2, X } from 'lucide-react';
import { useEffect, useState } from 'react';
import { GlassButton } from '@/components/ui/glass-button';
import { startUrlImport } from '@/lib/urlImport';
import type { ClipboardHit } from '@/lib/clipboardWatch';

interface ClipboardPromptProps {
  hit: ClipboardHit;
  onDismiss: () => void;
  onStarted: () => void;
}

/**
 * The one-click offer for a Bilibili link found on the clipboard.
 *
 * Deliberately a single, dismissible pill anchored above the bottom bar rather
 * than a modal: it must never interrupt, because the user did not ask for it. It
 * shows the id (not the title) because the title needs a network round trip, and
 * a prompt that waits for the network is not a one-click prompt.
 */
export function ClipboardPrompt({ hit, onDismiss, onStarted }: ClipboardPromptProps) {
  const { t } = useTranslation();
  const [isStarting, setIsStarting] = useState(false);
  const [error, setError] = useState('');

  // Escape dismisses, like any other transient surface.
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key === 'Escape') onDismiss();
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [onDismiss]);

  const handleImport = async () => {
    setIsStarting(true);
    setError('');
    try {
      // The same entry point the paste dialog uses, so the clipboard path gets
      // identical behaviour: same output dir, same formats, same translation
      // settings, same `url_import_*` events.
      await startUrlImport(hit.url);
      onStarted();
    } catch (startError) {
      setError(String(startError));
    } finally {
      setIsStarting(false);
    }
  };

  return (
    <div className="pointer-events-none absolute inset-x-0 bottom-24 z-40 flex justify-center px-6">
      <div
        className="pointer-events-auto flex max-w-[92%] items-center gap-3 rounded-2xl border border-glass-border-light dark:border-zinc-700 bg-white/90 dark:bg-zinc-900/90 px-4 py-3 shadow-glass-lg backdrop-blur-xl animate-fade-in"
        data-testid="clipboard-prompt"
        role="status"
      >
        <Link2 className="w-4 h-4 shrink-0 text-primary" />

        <div className="min-w-0">
          <p className="text-sm font-medium text-text-primary dark:text-white">
            {t('clipboardPrompt.title')}
          </p>
          <p
            className="truncate text-xs text-text-secondary dark:text-gray-400"
            title={hit.text}
            data-testid="clipboard-prompt-id"
          >
            {/* A short link has no id to show, so show the link instead of an
                opaque code that looks like one. */}
            {hit.id ?? hit.url}
          </p>
          {error && <p className="mt-0.5 text-xs text-danger">{error}</p>}
        </div>

        <GlassButton
          variant="primary"
          size="sm"
          onClick={() => void handleImport()}
          disabled={isStarting}
          data-testid="clipboard-prompt-import"
        >
          {isStarting ? <Loader2 className="w-4 h-4 animate-spin" /> : <Link2 className="w-4 h-4" />}
          {t('clipboardPrompt.import')}
        </GlassButton>

        <GlassButton
          variant="ghost"
          size="icon"
          onClick={onDismiss}
          className="h-7 w-7 shrink-0"
          aria-label={t('clipboardPrompt.dismiss')}
          data-testid="clipboard-prompt-dismiss"
        >
          <X className="w-4 h-4 text-text-tertiary dark:text-gray-500" />
        </GlassButton>
      </div>
    </div>
  );
}
