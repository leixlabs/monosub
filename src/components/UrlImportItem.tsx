import { useMemo, useState } from 'react';
import {
  AlertCircle,
  CheckCircle2,
  FolderOpen,
  Languages,
  Link2,
  Loader2,
  RotateCcw,
  ShieldAlert,
  Subtitles,
  X,
} from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { useStore, COOKIES_BROWSERS, type CookiesBrowser, type UrlImportState } from '@/store/useStore';
import { GlassCard } from '@/components/ui/glass-card';
import { GlassButton } from '@/components/ui/glass-button';
import { GlassBadge } from '@/components/ui/glass-badge';
import {
  cancelUrlImport,
  retryUrlImport,
  revealImportedFile,
} from '@/lib/urlImport';

interface UrlImportItemProps {
  urlImport: UrlImportState;
}

/** Stages that are still running, so the row shows a spinner and a cancel button. */
const ACTIVE_STAGES: UrlImportState['status'][] = [
  'pending',
  'resolving',
  'fetching_subtitles',
  'downloading_audio',
  'writing',
  'translating',
];

/**
 * One URL import row.
 *
 * Rendered above `FileList` and visually modelled on `FileItem`, but it is
 * deliberately a separate component: an import has no thumbnail, no size and no
 * engine step, and it must not participate in "start all" or the batch counters
 * (url-import-design §5.2). Once a `Queued` outcome arrives the row is removed and
 * the audio becomes an ordinary `FileItem`.
 */
export function UrlImportItem({ urlImport }: UrlImportItemProps) {
  const { t } = useTranslation();
  const settings = useStore((state) => state.settings);
  const updateSettings = useStore((state) => state.updateSettings);
  const [isRetrying, setIsRetrying] = useState(false);

  const isActive = ACTIVE_STAGES.includes(urlImport.status);
  const isDone = urlImport.status === 'done';
  const isError = urlImport.status === 'error';
  const needsCookies = Boolean(urlImport.needsCookies);

  const label = useMemo(() => {
    if (isDone) {
      if (urlImport.skipped) return t('urlImport.row.skipped');
      return urlImport.languageKind === 'ai'
        ? t('urlImport.row.subtitleAi', { language: urlImport.languageTag })
        : t('urlImport.row.subtitleHuman', { language: urlImport.languageTag });
    }
    if (isError) return urlImport.error || t('fileItem.error');
    return t(`urlImport.stage.${urlImport.status}`, urlImport.status);
  }, [isDone, isError, t, urlImport]);

  const firstWrittenPath = urlImport.paths?.[0] ?? urlImport.translatedPaths?.[0];

  const handleCancel = async () => {
    await cancelUrlImport(urlImport.taskId);
  };

  const handleRetry = async () => {
    setIsRetrying(true);
    try {
      await retryUrlImport(urlImport.taskId);
    } finally {
      setIsRetrying(false);
    }
  };

  const handleReveal = async () => {
    if (!firstWrittenPath) return;
    await revealImportedFile(firstWrittenPath);
  };

  const handleBrowserChange = (browser: CookiesBrowser) => {
    updateSettings({ bilibiliCookiesBrowser: browser });
  };

  return (
    <GlassCard
      glowEffect={isActive}
      className={`animate-fade-in ${isActive ? '' : 'hover:-translate-y-1 transition-transform duration-200'}`}
      data-testid="url-import-item"
    >
      <div className="flex gap-4 p-4 w-full h-full">
        {/* Icon plate, the visual counterpart of FileItem's thumbnail */}
        <div className="w-[140px] h-[80px] rounded-lg bg-bg-secondary dark:bg-zinc-800 flex-shrink-0 flex items-center justify-center relative">
          <Link2 className="w-8 h-8 text-primary" />
          {urlImport.languageTag && (
            <div className="absolute bottom-1.5 right-1.5 px-1.5 py-0.5 bg-black/70 dark:bg-black/80 rounded text-[10px] text-white font-medium">
              {urlImport.languageTag}
            </div>
          )}
        </div>

        {/* Details */}
        <div className="flex-1 flex flex-col justify-between min-w-0 py-0.5">
          <div>
            <h3
              className="text-sm font-medium text-text-primary dark:text-white truncate mb-1"
              title={urlImport.url}
            >
              {urlImport.input || urlImport.url}
            </h3>
            <div className="flex items-center gap-3 text-xs text-text-tertiary dark:text-gray-500">
              <span>{settings.outputFormat.toUpperCase()}</span>
              {isDone && urlImport.languageTag && (
                <>
                  <span className="w-1 h-1 rounded-full bg-text-tertiary dark:bg-gray-600" />
                  <GlassBadge variant={urlImport.languageKind === 'ai' ? 'default' : 'success'} size="sm">
                    <Subtitles className="w-3 h-3 mr-1" />
                    {urlImport.languageKind === 'ai' ? t('urlImport.row.ai') : t('urlImport.row.human')}
                  </GlassBadge>
                </>
              )}
              {isDone && (urlImport.translatedPaths?.length ?? 0) > 0 && (
                <>
                  <span className="w-1 h-1 rounded-full bg-text-tertiary dark:bg-gray-600" />
                  <span className="flex items-center gap-1">
                    <Languages className="w-3 h-3" />
                    {t('urlImport.row.translated', { language: settings.translateTargetLanguage })}
                  </span>
                </>
              )}
            </div>
          </div>

          <div className="mt-2 space-y-1.5">
            <div className="flex items-center gap-2">
              {isActive && <Loader2 className="w-3.5 h-3.5 text-primary animate-spin" />}
              {isDone && <CheckCircle2 className="w-3.5 h-3.5 text-success" />}
              {isError && <AlertCircle className="w-3.5 h-3.5 text-danger" />}
              <span
                className={`text-xs ${isError ? 'text-danger' : 'text-text-secondary dark:text-gray-400'}`}
                data-testid="url-import-status"
              >
                {label}
              </span>
            </div>

            {/* Progress. `undefined` means "size unknown", which renders as an
                indeterminate bar rather than a frozen 0%. */}
            {isActive && urlImport.status !== 'pending' && (
              <div className="w-full bg-divider dark:bg-zinc-700 rounded-full h-1.5 overflow-hidden">
                {typeof urlImport.progress === 'number' ? (
                  <div
                    className="bg-primary h-full rounded-full transition-[width] duration-300"
                    style={{ width: `${urlImport.progress}%` }}
                  />
                ) : (
                  <div className="h-full w-1/3 bg-primary/70 rounded-full animate-pulse-soft" />
                )}
              </div>
            )}

            {/* Login guidance (design §4.5). Only on the 412 path: it is the one
                failure a browser choice actually fixes. */}
            {isError && needsCookies && (
              <div className="p-2.5 rounded-xl border border-divider dark:border-zinc-700 bg-bg-secondary dark:bg-zinc-800/50 space-y-2">
                <p className="flex items-start gap-1.5 text-xs text-text-secondary dark:text-gray-400">
                  <ShieldAlert className="w-3.5 h-3.5 mt-0.5 shrink-0 text-warning" />
                  {t('urlImport.login.explain')}
                </p>
                <div className="flex flex-wrap items-center gap-1.5">
                  {COOKIES_BROWSERS.map((browser) => (
                    <GlassButton
                      key={browser}
                      variant={settings.bilibiliCookiesBrowser === browser ? 'primary' : 'outline'}
                      size="sm"
                      data-testid={`url-import-cookies-${browser}`}
                      onClick={() => handleBrowserChange(browser)}
                    >
                      {t(`urlImport.login.browsers.${browser}`)}
                    </GlassButton>
                  ))}
                </div>
                <p className="text-xxs text-text-tertiary dark:text-gray-500 leading-relaxed">
                  {t('urlImport.login.privacy')}
                </p>
                <p className="text-xxs text-text-tertiary dark:text-gray-500 leading-relaxed">
                  {t('urlImport.login.safariHint')}
                </p>
              </div>
            )}
          </div>
        </div>

        {/* Actions */}
        <div className="flex flex-col justify-center gap-2 w-[110px]">
          {isActive && urlImport.status !== 'pending' && (
            <GlassButton variant="destructive" size="sm" onClick={() => void handleCancel()} className="w-full">
              <X className="w-4 h-4" />
              {t('fileItem.cancel')}
            </GlassButton>
          )}
          {isDone && firstWrittenPath && (
            <GlassButton
              variant="default"
              size="sm"
              onClick={() => void handleReveal()}
              className="w-full text-success border-success/30 hover:bg-success/20"
              data-testid="url-import-reveal"
            >
              <FolderOpen className="w-4 h-4" />
              {t('fileItem.open')}
            </GlassButton>
          )}
          {isError && (
            <GlassButton
              variant="primary"
              size="sm"
              onClick={() => void handleRetry()}
              disabled={isRetrying}
              className="w-full"
              data-testid="url-import-retry"
            >
              {isRetrying ? <Loader2 className="w-4 h-4 animate-spin" /> : <RotateCcw className="w-4 h-4" />}
              {t('urlImport.row.retry')}
            </GlassButton>
          )}
        </div>
      </div>
    </GlassCard>
  );
}
