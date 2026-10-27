import { useEffect, useMemo, useState } from 'react';
import { Link2, Loader2, X } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { open } from '@tauri-apps/plugin-dialog';
import { useStore } from '@/store/useStore';
import { GlassButton } from '@/components/ui/glass-button';
import { GlassSwitch } from '@/components/ui/glass-switch';
import {
  loadDefaultUrlImportDir,
  parseInput,
  resolveUrlImportOutputDir,
  startUrlImports,
  subtitleLanguageForUi,
  type RejectedUrl,
} from '@/lib/urlImport';

interface UrlImportDialogProps {
  isOpen: boolean;
  onClose: () => void;
}

/**
 * The "add from a link" panel (url-import-design §5.2).
 *
 * Both read-only values follow a single source without a second control here:
 * the output directory mirrors Settings, and the subtitle language follows the
 * **interface** language. The subtitle is what the user reads, so a UI in
 * English asks for English captions; the recognition language in Settings
 * describes the audio and is untouched.
 */
export function UrlImportDialog({ isOpen, onClose }: UrlImportDialogProps) {
  const { t, i18n } = useTranslation();
  const settings = useStore((state) => state.settings);
  const updateSettings = useStore((state) => state.updateSettings);
  const autoStart = useStore((state) => state.autoStartImports);
  const setAutoStart = useStore((state) => state.setAutoStartImports);
  const storageMode = useStore((state) => state.storageMode);

  const [text, setText] = useState('');
  const [rejected, setRejected] = useState<RejectedUrl[]>([]);
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [outputDir, setOutputDir] = useState('');
  const [error, setError] = useState('');

  // The default directory comes from Rust (it knows the home dir); the custom
  // mode value comes straight from the store.
  useEffect(() => {
    if (!isOpen) return;
    let cancelled = false;
    void loadDefaultUrlImportDir()
      .then(() => {
        if (!cancelled) setOutputDir(resolveUrlImportOutputDir());
      })
      .catch((loadError) => {
        console.warn('[urlImport] could not resolve the default output directory', loadError);
      });
    return () => {
      cancelled = true;
    };
  }, [isOpen, settings.urlImportOutputDir, storageMode]);

  // The interface language, not the recognition setting: this is the language
  // the imported subtitle will be *read* in, so it has to track the UI.
  const uiLanguage = i18n.language;

  const languageLabel = useMemo(() => {
    const language = subtitleLanguageForUi(uiLanguage);
    const key = `language.${language}`;
    const translated = t(key, { defaultValue: '' });
    return translated || language;
  }, [t, uiLanguage]);

  if (!isOpen) return null;

  const handleSubmit = async () => {
    setError('');
    setRejected([]);
    setIsSubmitting(true);

    try {
      const parsed = await parseInput(text);
      setRejected(parsed.rejected);

      if (parsed.accepted.length === 0) {
        // Everything was rejected; keep the panel open so the rows stay visible.
        return;
      }

      await startUrlImports(parsed.accepted);
      setText('');
      onClose();
    } catch (submitError) {
      setError(String(submitError));
    } finally {
      setIsSubmitting(false);
    }
  };

  const handleChooseDir = async () => {
    try {
      const selected = await open({ directory: true, multiple: false });
      if (typeof selected === 'string') {
        updateSettings({ urlImportOutputDir: selected });
        setOutputDir(selected);
      }
    } catch (chooseError) {
      console.error('[urlImport] could not choose an output directory', chooseError);
    }
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center">
      <div
        className="absolute inset-0 bg-black/40 dark:bg-black/60 backdrop-blur-xl"
        onClick={onClose}
      />

      <div
        className="relative glass-medium dark:glass-dark-strong rounded-3xl shadow-glass-lg w-[560px] max-w-[92vw] overflow-hidden flex flex-col"
        data-testid="url-import-dialog"
      >
        <div className="flex items-center justify-between px-6 py-4 border-b border-glass-border-light dark:border-glass-border-dark">
          <h2 className="flex items-center gap-2 text-base font-bold text-text-primary dark:text-white">
            <Link2 className="w-4 h-4 text-primary" />
            {t('urlImport.title')}
          </h2>
          <GlassButton variant="ghost" size="icon" onClick={onClose} className="h-8 w-8">
            <X className="w-5 h-5 text-text-primary dark:text-gray-300" />
          </GlassButton>
        </div>

        <div className="px-6 py-5 space-y-4">
          <div className="space-y-1.5">
            <textarea
              value={text}
              onChange={(event) => setText(event.target.value)}
              placeholder={t('urlImport.placeholder')}
              spellCheck={false}
              rows={5}
              data-testid="url-import-input"
              className="w-full resize-y rounded-xl border border-divider dark:border-zinc-700 bg-bg-secondary dark:bg-zinc-800/60 px-3 py-2 text-sm text-text-primary dark:text-gray-200 placeholder:text-text-tertiary dark:placeholder:text-gray-500 focus:outline-none focus:ring-2 focus:ring-primary/40 font-mono"
            />
            <p className="text-xs text-text-secondary dark:text-gray-400">{t('urlImport.hint')}</p>
          </div>

          {rejected.length > 0 && (
            <div
              className="space-y-1 p-2.5 rounded-xl border border-danger/30 bg-danger/5"
              data-testid="url-import-errors"
            >
              {rejected.map((row, index) => (
                <p
                  key={`${row.input}-${index}`}
                  className="text-xs text-danger break-all"
                  data-testid="url-import-error-row"
                >
                  {t(`urlImport.errors.${row.error}`)}: <span className="font-mono">{row.input}</span>
                </p>
              ))}
            </div>
          )}

          {/* Read-only values with one source of truth each (§5.2). */}
          <div className="grid grid-cols-1 gap-2 text-xs">
            <div className="flex items-center justify-between gap-3">
              <span className="text-text-secondary dark:text-gray-400">{t('urlImport.subtitleLanguage')}</span>
              <span className="text-text-primary dark:text-gray-200 font-medium">
                {languageLabel}
              </span>
            </div>
            <p className="text-[11px] text-text-tertiary dark:text-gray-500 -mt-0.5">
              {t('urlImport.followsInterfaceLanguage')}
            </p>
            <div className="flex items-center justify-between gap-3">
              <span className="shrink-0 text-text-secondary dark:text-gray-400">{t('urlImport.outputDir')}</span>
              <button
                type="button"
                onClick={() => void handleChooseDir()}
                className="min-w-0 truncate font-mono text-text-primary dark:text-gray-200 hover:text-primary text-right"
                title={outputDir || undefined}
              >
                {outputDir || t('urlImport.outputDirDefault')}
              </button>
            </div>
          </div>

          <div className="flex items-center justify-between">
            <div>
              <label className="text-sm font-medium text-text-primary dark:text-gray-200">
                {t('urlImport.autoStart')}
              </label>
              <p className="text-xs text-text-secondary dark:text-gray-400 mt-0.5">
                {t('urlImport.autoStartHint')}
              </p>
            </div>
            <GlassSwitch
              checked={autoStart}
              onCheckedChange={setAutoStart}
              data-testid="url-import-autostart"
            />
          </div>

          {error && <p className="text-xs text-danger">{error}</p>}
        </div>

        <div className="flex items-center justify-end gap-2 px-6 py-4 border-t border-glass-border-light dark:border-glass-border-dark">
          <GlassButton variant="ghost" size="sm" onClick={onClose}>
            {t('urlImport.cancel')}
          </GlassButton>
          <GlassButton
            variant="primary"
            size="sm"
            onClick={() => void handleSubmit()}
            disabled={isSubmitting || text.trim().length === 0}
            data-testid="url-import-submit"
          >
            {isSubmitting ? <Loader2 className="w-4 h-4 animate-spin" /> : <Link2 className="w-4 h-4" />}
            {t('urlImport.submit')}
          </GlassButton>
        </div>
      </div>
    </div>
  );
}
