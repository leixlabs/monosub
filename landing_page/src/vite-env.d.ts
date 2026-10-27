/// <reference types="vite/client" />

interface ImportMetaEnv {
  /**
   * Public origin of the site, no trailing slash. Baked in at build time by
   * `scripts/prerender.mjs` and `vite build`; falls back to the current domain
   * in `src/lib/site.ts` when unset.
   */
  readonly VITE_SITE_URL?: string;
}

interface ImportMeta {
  readonly env: ImportMetaEnv;
}
