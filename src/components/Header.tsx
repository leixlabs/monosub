import { Settings as SettingsIcon, Subtitles } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { useStore } from '@/store/useStore';
import { ThemeToggle } from './ThemeToggle';
import { GlassButton } from '@/components/ui/glass-button';
import { GlassTooltip } from '@/components/ui/glass-tooltip';
import { WindowControls } from '@/components/WindowControls';
import { detectPlatform } from '@/lib/nativeWindow';

/**
 * Custom title bar. The whole bar is a drag region; interactive children opt out
 * with `no-drag`. `pl-20` keeps clear of the macOS traffic lights (Overlay style).
 * Translucent `chrome` surface + hairline bottom border gives the native
 * vibrancy look and separates the bar from scrolling content.
 */
export function Header() {
  const { t } = useTranslation();
  const setIsSettingsOpen = useStore((state) => state.setIsSettingsOpen);
  const files = useStore((state) => state.files);
  const processedCount = files.filter((f) => f.status === 'completed').length;
  const totalCount = files.length;
  const percent = totalCount > 0 ? (processedCount / totalCount) * 100 : 0;
  const platform = detectPlatform();
  const isWindows = platform === 'windows';

  return (
    <header
      data-tauri-drag-region
      className={`drag-region chrome h-12 flex items-center gap-3 ${
        // macOS: clear the traffic lights. Windows: content starts at the edge and
        // our own caption buttons sit flush right.
        isWindows ? 'pl-4 pr-4' : 'pl-20 pr-4'
      } sticky top-0 z-40 border-b border-line transition-colors`}
    >
      {/* Brand mark */}
      <div data-tauri-drag-region className="flex items-center gap-2.5 pointer-events-none">
        <span className="grid place-items-center w-6 h-6 rounded-md bg-primary text-primary-foreground shadow-[0_1px_2px_rgba(16,24,40,0.2)]">
          <Subtitles className="w-3.5 h-3.5" strokeWidth={2.25} />
        </span>
        <h1 className="window-title text-[15px] font-semibold tracking-tight">{t('app.name')}</h1>
      </div>

      <div data-tauri-drag-region className="flex-1 h-full" />

      {/* self-stretch: the caption buttons must span the full bar height, so this
          group takes the header's height instead of centring at content height. */}
      <div className="flex items-center gap-1 no-drag self-stretch">
        {totalCount > 0 && (
          <div
            className="flex items-center gap-2 mr-2 px-2.5 h-7 rounded-md bg-black/[0.04] dark:bg-white/[0.06]"
            role="status"
            aria-label={`${processedCount}/${totalCount} ${t('header.completed')}`}
          >
            <span className="text-xs tabular-nums text-text-secondary dark:text-zinc-400">
              <span className="font-medium text-text-primary dark:text-zinc-100">{processedCount}</span>
              <span className="opacity-60">/{totalCount}</span> {t('header.completed')}
            </span>
            <div className="w-14 h-1 rounded-full bg-black/10 dark:bg-white/10 overflow-hidden">
              <div
                className="h-full rounded-full bg-primary transition-[width] duration-500 ease-out-expo"
                style={{ width: `${percent}%` }}
              />
            </div>
          </div>
        )}
        <ThemeToggle />
        <GlassTooltip content={t('header.settings')} side="bottom">
          <GlassButton
            variant="ghost"
            size="icon"
            onClick={() => setIsSettingsOpen(true)}
            aria-label={t('header.settings')}
            className="no-drag"
          >
            <SettingsIcon className="w-[18px] h-[18px]" />
          </GlassButton>
        </GlassTooltip>
        {isWindows && <WindowControls />}
      </div>
    </header>
  );
}
