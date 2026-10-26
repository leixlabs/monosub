import { Effect, getCurrentWindow } from '@tauri-apps/api/window';

export type Platform = 'macos' | 'windows' | 'other';

/** Synchronous, so the first paint already has the right title-bar layout. */
export function detectPlatform(): Platform {
  if (typeof navigator === 'undefined') return 'other';
  const ua = navigator.userAgent;
  if (/Windows/i.test(ua)) return 'windows';
  if (/Macintosh|Mac OS X/i.test(ua)) return 'macos';
  return 'other';
}

const isTauri = () =>
  typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;

/**
 * Give the window its native material (Mica on Windows 11, system material on macOS)
 * and, only if that succeeded, mark `<html class="vibrancy">` so the page background
 * becomes transparent and the material shows through.
 *
 * Order matters: making the page transparent *before* the OS layer exists would
 * flash a black/white window, so the class is added after `setEffects` resolves.
 * On failure (older OS, Linux, browser dev) nothing changes and the opaque
 * fallback surfaces stay in place.
 */
export async function applyNativeMaterial(): Promise<boolean> {
  const platform = detectPlatform();
  document.documentElement.dataset.platform = platform;
  if (!isTauri() || platform === 'other') return false;

  try {
    const win = getCurrentWindow();
    await win.setEffects({
      // First supported effect wins: Mica on Win11, blur on Win10, vibrancy on macOS.
      effects:
        platform === 'windows'
          ? [Effect.Mica, Effect.Blur]
          : [Effect.UnderWindowBackground],
    });
    document.documentElement.classList.add('vibrancy');
    return true;
  } catch (error) {
    console.warn('[nativeWindow] window effects unavailable, using opaque surfaces:', error);
    return false;
  }
}
