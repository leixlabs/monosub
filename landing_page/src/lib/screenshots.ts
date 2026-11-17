/**
 * Product screenshots captured from the current MonoSub build (macOS App Store
 * 2.1.0), in both UI languages (English + Simplified Chinese) and both themes.
 *
 * Source captures were 2560×1600; downscaled to 1600×1000 WebP. Sizes are passed
 * to <img> so layout is reserved before the image loads (no CLS).
 *
 * The page picks the pair that matches its own locale — `/en/` loads the English
 * UI, `/zh/` the Chinese UI — so a Chinese page never shows an English window.
 */

const SHOTS = {
  'main-en-light': { width: 1600, height: 1000 },
  'main-en-dark': { width: 1600, height: 1000 },
  'main-zh-light': { width: 1600, height: 1000 },
  'main-zh-dark': { width: 1600, height: 1000 },
  'settings-en-light': { width: 1600, height: 1000 },
  'settings-en-dark': { width: 1600, height: 1000 },
  'settings-zh-light': { width: 1600, height: 1000 },
  'settings-zh-dark': { width: 1600, height: 1000 },
} as const;

export type ShotName = keyof typeof SHOTS;

export const shot = (name: ShotName): string => `/screenshots/${name}.webp`;

export const shotSize = (name: ShotName): { width: number; height: number } => ({
  width: SHOTS[name].width,
  height: SHOTS[name].height,
});

/**
 * Hero screenshot matched to the page locale (light theme).
 *
 * The captures are of the macOS window, so a Windows visitor would be shown
 * traffic-light buttons they will never see. There is no Windows capture yet, so
 * `platform` is accepted for that case and currently ignored — the honest fix is
 * a Windows capture, not a cropped Mac one.
 */
export const heroShot = (lang: string): ShotName =>
  lang.startsWith('zh') ? 'main-zh-light' : 'main-en-light';
