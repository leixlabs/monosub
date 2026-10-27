import type { ImgHTMLAttributes } from 'react';

/**
 * `fetchpriority` as a spread, not as a JSX prop.
 *
 * React 18 does not know this attribute: writing `fetchPriority="high"` makes it
 * warn ("React does not recognize the fetchPriority prop") and emit the
 * camelCase name into the HTML, which browsers ignore — so the hint is lost and
 * the console fills with noise. React 19 added the prop; until this project
 * upgrades, spreading the lowercase attribute passes it through verbatim, which
 * is exactly what the HTML spec asks for.
 */
export const highPriorityImage = {
  fetchpriority: 'high',
} as unknown as ImgHTMLAttributes<HTMLImageElement>;

/** Same, for images below the fold that should not compete with the LCP one. */
export const lowPriorityImage = {
  fetchpriority: 'low',
  loading: 'lazy',
  decoding: 'async',
} as unknown as ImgHTMLAttributes<HTMLImageElement>;
