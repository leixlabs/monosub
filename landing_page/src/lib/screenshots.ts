/**
 * Product screenshots, taken from the current app (macOS App Store 2.1.0 build,
 * 2560×1600 captures downscaled to 1600×1000 WebP).
 *
 * Six states, light and dark: the empty main window, a batch being processed, and
 * Settings. Every image is a real capture of MonoSub — nothing is a mock-up.
 *
 * The App Store's Chinese listing was uploaded with the English-UI captures, so
 * a single set serves both locales; the interface language is a user setting, not
 * a property of the product shot. If a fully Chinese-UI set is captured later,
 * add a locale dimension back here.
 *
 * Sizes are passed to <img> so the layout is reserved before the image arrives
 * (no CLS).
 */

const SHOTS = {
  'main-light': { width: 1600, height: 1000 },
  'main-dark': { width: 1600, height: 1000 },
  'working-light': { width: 1600, height: 1000 },
  'working-dark': { width: 1600, height: 1000 },
  'setting-light': { width: 1600, height: 1000 },
  'setting-dark': { width: 1600, height: 1000 },
} as const;

export type ShotName = keyof typeof SHOTS;

export const shot = (name: ShotName): string => `/screenshots/${name}.webp`;

export const shotSize = (name: ShotName): { width: number; height: number } => ({
  width: SHOTS[name].width,
  height: SHOTS[name].height,
});
