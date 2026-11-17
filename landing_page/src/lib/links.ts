/**
 * Where the app can be obtained. Four channels, kept in one place so a link can
 * never go stale in one component and not another.
 *
 * The product changed from "one paid app" to "free download + one-time Pro
 * unlock", so there is no longer a single STORE_URL: macOS lives on the App
 * Store (free download, Pro sold as an in-app purchase), Windows is still a
 * one-time paid app on the Microsoft Store, and the direct checkout sells an
 * offline license that works on either platform. GitHub Releases still ships the
 * free macOS build from source.
 */

/** macOS — App Store. Free download; Pro is a one-time in-app purchase. */
export const APP_STORE_URL = 'https://apps.apple.com/us/app/monosub/id6829689694';

/** Direct store — MonoSub Pro offline license (¥36 / $8.60), works on both OSes. */
export const CHECKOUT_URL = 'https://checkout.leix.dev/p/monosub-pro';

/** Windows — Microsoft Store. One-time purchase, Pro included. */
export const MS_STORE_URL = 'https://apps.microsoft.com/detail/9P6WTCCRQTVW';

/** Free macOS build from GitHub Releases. */
export const RELEASES_URL = 'https://github.com/leixlabs/monosub/releases/latest';

export const REPO_URL = 'https://github.com/leixlabs/monosub';

/**
 * The primary "download" destination on the hero. macOS first because it is the
 * platform with a clean free download + IAP flow; the pricing section spells out
 * every channel for people on Windows or who want the direct license.
 */
export const PRIMARY_DOWNLOAD_URL = APP_STORE_URL;

/**
 * The primary download destination for a platform.
 *
 * - `windows` → Microsoft Store (the only Windows channel).
 * - `mac`     → Mac App Store (free download + Pro in-app purchase).
 * - `unknown` → the in-page pricing table, which lists every channel. Sending a
 *   Linux or unidentified visitor straight to the Mac App Store would be a lie
 *   about what they can install, and there is no Linux build to offer instead.
 */
export const downloadUrlFor = (platform: 'mac' | 'windows' | 'unknown'): string => {
  if (platform === 'windows') return MS_STORE_URL;
  if (platform === 'mac') return APP_STORE_URL;
  return '#pricing';
};
