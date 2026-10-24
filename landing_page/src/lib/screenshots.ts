/**
 * Product screenshots, captured from the running app.
 *
 * Every shot exists twice — `-zh-CN` and `-en` — because the app's own UI is
 * localised, so an English page showing a Chinese window would be a mismatch.
 * The files live in `public/screenshots/` as WebP.
 *
 * A Settings shot is cropped to the modal (the window around it is empty), so
 * its aspect ratio is not the 3:2 of a full-window capture. Each entry therefore
 * carries its own intrinsic size, which is passed to <img> so the layout is
 * reserved before the image arrives and never shifts.
 */

const SHOTS = {
  queue: { file: '02-queue', width: 1600, height: 1067 },
  batchRunning: { file: '08-batch-running', width: 1600, height: 1067 },
  rowStates: { file: '07-row-states', width: 1600, height: 1067 },
  translated: { file: '09-translated-row', width: 1600, height: 1067 },
  clipboardPill: { file: '04-clipboard-pill', width: 1600, height: 1067 },
  urlImportRows: { file: '33-url-import-rows', width: 1600, height: 1067 },
  urlDialog: { file: '22-url-dialog-valid', width: 1600, height: 1067 },
  models: { file: '11-settings-models-top', width: 1600, height: 1352 },
  modelCatalogue: { file: '27-models-installed', width: 1600, height: 1352 },
  translationSettings: { file: '12-settings-translate-top', width: 1600, height: 1352 },
  lightTheme: { file: '40-light-theme', width: 1600, height: 1067 },
} as const;

export type ShotName = keyof typeof SHOTS;

/** The landing page ships `zh` (not `zh-CN`) as its route; the files use `zh-CN`. */
const fileLocale = (lang: string) => (lang.startsWith('zh') ? 'zh-CN' : 'en');

export const shot = (name: ShotName, lang: string): string =>
  `/screenshots/${SHOTS[name].file}-${fileLocale(lang)}.webp`;

export const shotSize = (name: ShotName): { width: number; height: number } => ({
  width: SHOTS[name].width,
  height: SHOTS[name].height,
});
