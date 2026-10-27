/**
 * The single source of truth for the site's public origin.
 *
 * Every canonical / hreflang / og:url / sitemap URL is derived from this value,
 * so moving to a new domain is one edit (or one build-time env var) rather than
 * a grep across components:
 *
 *   VITE_SITE_URL=https://monosub.app bun run build
 *
 * Set VITE_SITE_URL in the Cloudflare Pages project (Settings → Environment
 * variables) when the production domain changes; the fallback below keeps a
 * plain `bun run build` working against the current domain.
 */

const RAW_SITE_URL = import.meta.env.VITE_SITE_URL as string | undefined;

/** Origin only — no trailing slash, no path. */
export const SITE_URL = (RAW_SITE_URL?.trim() || 'https://monosub.leix.dev').replace(/\/+$/, '');

/**
 * Absolute URL for a site-relative path. Paths are written with the leading
 * slash the router uses (`/zh`, `/zh/privacy`); the canonical form keeps the
 * trailing slash the pages are actually served under.
 */
export const absoluteUrl = (path: string): string => `${SITE_URL}${path}`;
