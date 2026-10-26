import { useEffect, useState } from 'react';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { useTranslation } from 'react-i18next';
import { cn } from '@/lib/utils';

/**
 * Windows 11–style caption buttons (46×full-height, flat, red close).
 *
 * Only rendered on Windows, where the native frame is off (`decorations: false`).
 * macOS keeps its real traffic lights.
 */

const glyph = 'w-[10px] h-[10px]';

function Minimize() {
  return (
    <svg viewBox="0 0 10 10" className={glyph} aria-hidden="true">
      <path d="M0 5.5h10" stroke="currentColor" strokeWidth="1" />
    </svg>
  );
}
function Maximize() {
  return (
    <svg viewBox="0 0 10 10" className={glyph} aria-hidden="true">
      <rect x="0.5" y="0.5" width="9" height="9" rx="1" fill="none" stroke="currentColor" strokeWidth="1" />
    </svg>
  );
}
function Restore() {
  return (
    <svg viewBox="0 0 10 10" className={glyph} aria-hidden="true">
      <rect x="0.5" y="2.5" width="7" height="7" rx="1" fill="none" stroke="currentColor" strokeWidth="1" />
      <path d="M2.5 2.5V1.5a1 1 0 0 1 1-1h5a1 1 0 0 1 1 1v5a1 1 0 0 1-1 1h-1" fill="none" stroke="currentColor" strokeWidth="1" />
    </svg>
  );
}
function Close() {
  return (
    <svg viewBox="0 0 10 10" className={glyph} aria-hidden="true">
      <path d="M0.5 0.5l9 9M9.5 0.5l-9 9" stroke="currentColor" strokeWidth="1" strokeLinecap="round" />
    </svg>
  );
}

const base =
  'no-drag grid place-items-center w-[46px] self-stretch text-text-primary dark:text-zinc-100 ' +
  'transition-colors duration-150 focus-visible:outline-none focus-visible:bg-black/10 dark:focus-visible:bg-white/15';

export function WindowControls() {
  const { t } = useTranslation();
  const [maximized, setMaximized] = useState(false);

  useEffect(() => {
    const win = getCurrentWindow();
    let disposed = false;
    let unlisten: (() => void) | undefined;

    const sync = () => void win.isMaximized().then((m) => !disposed && setMaximized(m));
    sync();
    void win.onResized(sync).then((fn) => {
      if (disposed) fn();
      else unlisten = fn;
    });
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, []);

  const win = getCurrentWindow();

  return (
    <div className="flex self-stretch -mr-4 ml-2" role="group" aria-label="Window controls">
      <button
        type="button"
        className={cn(base, 'active:scale-100 hover:bg-black/[0.06] dark:hover:bg-white/10 active:bg-black/10 dark:active:bg-white/15')}
        onClick={() => void win.minimize()}
        aria-label={t('window.minimize', 'Minimize')}
      >
        <Minimize />
      </button>
      <button
        type="button"
        className={cn(base, 'active:scale-100 hover:bg-black/[0.06] dark:hover:bg-white/10 active:bg-black/10 dark:active:bg-white/15')}
        onClick={() => void win.toggleMaximize()}
        aria-label={maximized ? t('window.restore', 'Restore') : t('window.maximize', 'Maximize')}
      >
        {maximized ? <Restore /> : <Maximize />}
      </button>
      <button
        type="button"
        // Top-right radius matches the Win11 window corner (8px) so the red
        // fill follows the frame instead of poking out of it. Also cancels the
        // global `button:active` scale, which would shrink the hit area mid-click.
        className={cn(base, 'rounded-tr-[8px] active:scale-100 hover:bg-[#C42B1C] hover:text-white active:bg-[#C42B1C]/80 active:text-white/80')}
        onClick={() => void win.close()}
        aria-label={t('window.close', 'Close')}
      >
        <Close />
      </button>
    </div>
  );
}
