/**
 * Post-build step: turn the Vite SPA shell into real static HTML.
 *
 * Why this exists: the site is React + Vite, so `dist/index.html` was a 785-byte
 * shell with an empty `<div id="root">`. Google will eventually render it, but
 * Baidu largely will not — and Baidu is the point for the Chinese page. So every
 * URL that has a page is rendered here, once, at build time.
 *
 * The same pass owns everything that has to agree with the domain and the URL
 * shape:
 *   - `sitemap.xml` with hreflang alternates
 *   - `robots.txt`
 *   - `404.html` (Pages serves this with a real 404 status)
 *   - `_redirects`: legacy locales, and slashless URLs onto their canonical form
 *
 * Domain changes go through `VITE_SITE_URL`; nothing here hardcodes a host.
 */

import { build } from 'esbuild';
import { mkdir, readFile, rm, writeFile } from 'node:fs/promises';
import { existsSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { createRequire } from 'node:module';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const dist = join(root, 'dist');
const ssrDir = join(root, 'node_modules', '.monosub-prerender');
const ssrEntry = join(ssrDir, 'entry-server.cjs');

if (!existsSync(join(dist, 'index.html'))) {
  console.error('[prerender] dist/index.html not found — run `vite build` first.');
  process.exit(1);
}

// ── 1. Bundle the server entry ────────────────────────────────────────────────
// Emitted as CommonJS, not ESM, and that choice is load-bearing: react-dom/server
// and react-helmet-async are CommonJS packages, and an ESM bundle either turns
// their dynamic `require('stream')` into a runtime error or breaks their named
// exports. As CJS, esbuild leaves Node builtins as plain `require` calls and the
// interop is exactly what those packages expect.
await mkdir(ssrDir, { recursive: true });
await build({
  entryPoints: [join(root, 'src', 'entry-server.tsx')],
  outfile: ssrEntry,
  bundle: true,
  platform: 'node',
  format: 'cjs',
  target: 'node20',
  jsx: 'automatic',
  logLevel: 'warning',
  // Vite normally substitutes this; reproduce it so the server render agrees
  // with the client bundle about the site origin.
  define: {
    'import.meta.env.VITE_SITE_URL': JSON.stringify(process.env.VITE_SITE_URL ?? ''),
  },
});

const { render, PRERENDER_PAGES, SITEMAP_PAGES, LEGACY_LOCALES, SITE_URL } = createRequire(
  import.meta.url
)(ssrEntry);

// ── 2. Render every page into the Vite shell ─────────────────────────────────
// Read once, up front: the `/` page overwrites dist/index.html, and every other
// page must reuse the pristine template rather than the already-rendered one.
const template = await readFile(join(dist, 'index.html'), 'utf8');

/** Drop the shell's fallback <title>/<meta description>; Helmet supplies real ones. */
const shellHeadPattern =
  /^[ \t]*<title>[\s\S]*?<\/title>\r?\n|^[ \t]*<meta name="description"[^>]*>\r?\n/gm;

const inject = (templateHtml, { body, head, htmlAttributes }) => {
  let out = templateHtml.replace(shellHeadPattern, '');

  // Carry Helmet's <html> attributes over the shell's hardcoded lang="en".
  out = out.replace(/<html[^>]*>/, `<html ${htmlAttributes || 'lang="en"'}>`);
  out = out.replace('<div id="root"></div>', `<div id="root">${body}</div>`);
  out = out.replace('</head>', `    ${head}\n  </head>`);

  return out;
};

let rendered = 0;
for (const page of PRERENDER_PAGES) {
  const html = inject(template, render(page.url));

  const outPath = join(dist, page.outFile);
  await mkdir(dirname(outPath), { recursive: true });
  await writeFile(outPath, html, 'utf8');
  rendered += 1;
  console.log(
    `[prerender] ${page.url} → dist/${page.outFile} (${(html.length / 1024).toFixed(1)} kB)`
  );
}

// ── 3. 404.html ──────────────────────────────────────────────────────────────
// Without this file Pages assumes an SPA and answers every unknown path with the
// root shell and a 200 — a soft 404, which keeps junk URLs "valid" forever.
// Rendered through the same component tree as the client route so the page a
// crawler sees and the page an in-app navigation shows are the same page.
//
// The URL is two segments deep on purpose: `/:lang` would swallow a single-segment
// path like `/__not-found__` and render the landing page into the 404 file.
const notFoundHtml = inject(template, render('/404/page-not-found'));
await writeFile(join(dist, '404.html'), notFoundHtml, 'utf8');
console.log('[prerender] dist/404.html');

// ── 4. sitemap.xml ───────────────────────────────────────────────────────────
const escapeXml = (value) =>
  value
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;');

const today = new Date().toISOString().slice(0, 10);

const sitemap = `<?xml version="1.0" encoding="UTF-8"?>
<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9"
        xmlns:xhtml="http://www.w3.org/1999/xhtml">
${SITEMAP_PAGES.map(
  (page) => `  <url>
    <loc>${escapeXml(page.loc)}</loc>
    <lastmod>${today}</lastmod>
    <changefreq>weekly</changefreq>
    <priority>1.0</priority>
${page.alternates
  .map(
    (alt) =>
      `    <xhtml:link rel="alternate" hreflang="${alt.hrefLang}" href="${escapeXml(alt.href)}"/>`
  )
  .join('\n')}
    <xhtml:link rel="alternate" hreflang="x-default" href="${escapeXml(page.xDefault)}"/>
  </url>`
).join('\n')}
</urlset>
`;

await writeFile(join(dist, 'sitemap.xml'), sitemap, 'utf8');
console.log(`[prerender] dist/sitemap.xml (${SITEMAP_PAGES.length} urls)`);

// ── 5. robots.txt ────────────────────────────────────────────────────────────
// `/privacy` is noindex, and there is no other private area worth hiding, so this
// stays maximally open — the sitemap line is the part that matters.
const robots = `User-agent: *
Allow: /

Sitemap: ${SITE_URL}/sitemap.xml
`;

await writeFile(join(dist, 'robots.txt'), robots, 'utf8');
console.log('[prerender] dist/robots.txt');

// ── 6. _redirects ────────────────────────────────────────────────────────────
// Three jobs, in this order of importance:
//
//   1. Legacy locales. The multilingual release published /ja/, /de/, … and those
//      URLs are still in the index. They have no page now, so without an explicit
//      redirect they 404 and drop whatever equity they carry.
//   2. Slashless → slashed. Pages serves both `/zh` and `/zh/`, which is duplicate
//      content with no canonical to settle it at the HTTP level; pinning the
//      slashless form onto the canonical one removes the ambiguity.
//   3. `/zh-tw/` folds into `/zh/`, not English — it is Chinese content.
const redirectLines = [
  '# Generated by landing_page/scripts/prerender.mjs — do not edit dist/_redirects.',
  '',
  '# Legacy locale URLs from the multilingual release',
  '/zh-tw/*   /zh/   301',
  '/zh-tw     /zh/   301',
  ...LEGACY_LOCALES.flatMap((locale) => [
    `/${locale}/*   /en/   301`,
    `/${locale}     /en/   301`,
  ]),
  '',
  '# Blog links that arrived without a locale',
  '# (`/blog/x` would otherwise look like a language code named "blog")',
  '/blog/*   /en/blog/:splat   301',
  '/blog     /en/blog/   301',
  '',
  '# Slashless → canonical (trailing slash) form',
  ...PRERENDER_PAGES.filter((page) => page.url !== '/').map(
    (page) => `${page.url.replace(/\/$/, '')}   ${page.url}   301`
  ),
];

await writeFile(join(dist, '_redirects'), `${redirectLines.join('\n')}\n`, 'utf8');
console.log(
  `[prerender] dist/_redirects (${LEGACY_LOCALES.length + 2} legacy locales + slash normalisation)`
);

// ── 7. Clean up the temporary server bundle ─────────────────────────────────
await rm(ssrDir, { recursive: true, force: true });

console.log(`\n[prerender] done — ${rendered} pages + 404.html, site origin ${SITE_URL}`);
