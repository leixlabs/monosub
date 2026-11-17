/**
 * Which platform the visitor is on, so the primary "download" button can point at
 * the right store instead of always sending Windows users to the Mac App Store.
 *
 * Detection is deliberately conservative: only `mac` and `windows` get a
 * platform-specific destination. Everything else (Linux, ChromeOS, iOS, Android,
 * an unknown or privacy-masked UA) falls back to `unknown`, which keeps the
 * button pointing at the pricing table where every channel is listed. Guessing
 * wrong is worse than not guessing.
 *
 * iPadOS 13+ reports a desktop `Macintosh` UA, so a touch-capable "Mac" is
 * treated as a tablet rather than a Mac — the App Store link would open the iOS
 * listing, which is not this product.
 */

export type Platform = 'mac' | 'windows' | 'unknown';

export const detectPlatform = (): Platform => {
  if (typeof navigator === 'undefined') return 'unknown';

  // User-Agent Client Hints are the modern, spoof-resistant source where present.
  const uaData = (navigator as Navigator & { userAgentData?: { platform?: string } })
    .userAgentData;
  const hint = uaData?.platform?.toLowerCase() ?? '';

  const ua = navigator.userAgent ?? '';
  const platform = (navigator.platform ?? '').toLowerCase();

  const isIPad = /ipad/.test(ua) || (platform === 'macintel' && navigator.maxTouchPoints > 1);
  if (isIPad || /iphone|ipod|android/i.test(ua)) return 'unknown';

  if (hint) {
    if (hint.includes('mac')) return 'mac';
    if (hint.includes('windows')) return 'windows';
    return 'unknown';
  }

  if (/mac/i.test(platform) || /macintosh/i.test(ua)) return 'mac';
  if (/win/i.test(platform) || /windows/i.test(ua)) return 'windows';
  return 'unknown';
};
