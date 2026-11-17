import { useEffect, useState } from 'react';
import { detectPlatform, type Platform } from '../lib/platform';

/**
 * The visitor's platform, resolved after mount.
 *
 * It starts as `unknown` on purpose — the same value the prerenderer produces —
 * so the server HTML and the first client render agree and hydration never
 * mismatches. The effect then swaps in the detected platform, which only changes
 * a label and a link, never layout, so the swap is not visible as a shift.
 *
 * Consequence worth knowing: the static HTML that crawlers see has the generic
 * "all download options" CTA. That is the honest answer for a crawler, and the
 * pricing table it points at lists every channel.
 */
export const usePlatform = (): Platform => {
  const [platform, setPlatform] = useState<Platform>('unknown');

  useEffect(() => {
    setPlatform(detectPlatform());
  }, []);

  return platform;
};
