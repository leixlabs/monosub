import { absoluteUrl } from './site';

/**
 * Canonical URLs for the two pages that exist, in both shipped locales.
 *
 * The trailing-slash form is canonical because it is the URL the host actually
 * serves: the prerenderer writes `zh/index.html`, and Cloudflare Pages answers a
 * directory-index request at `/zh/` (a slashless `/zh` gets a 308 onto it). A
 * canonical pointing at the redirecting form would be a self-inflicted wound, so
 * `_redirects` normalises the other direction and every internal link uses
 * `pagePath()` — one URL per page, everywhere.
 */

export type PageName = 'home' | 'privacy';

const PAGE_PATHS: Record<PageName, string> = {
  home: '/',
  privacy: '/privacy/',
};

/**
 * The blog lives under `/blog/` with the locale in front (`/zh/blog/`), rather
 * than publishing posts at `/<lang>/<slug>/`. Two reasons: a post slug can never
 * then collide with a page route, and the whole comparison series stays
 * groupable — which is also how a reader thinks about it.
 *
 * Both helpers return the trailing-slash form because that is what a static host
 * actually serves (`zh/blog/index.html`), and what `_redirects` normalises the
 * slashless variant onto. Links, canonicals and the sitemap all use them, so no
 * internal link points at a redirect.
 */
export const BLOG_PATH: Record<'zh' | 'en', string> = {
  zh: '/zh/blog/',
  en: '/en/blog/',
};

export const blogIndexPath = (locale: 'zh' | 'en'): string => BLOG_PATH[locale];

export const blogPostPath = (locale: 'zh' | 'en', slug: string): string =>
  `${BLOG_PATH[locale]}${slug}/`;

export const blogIndexUrl = (locale: 'zh' | 'en'): string =>
  absoluteUrl(blogIndexPath(locale));

export const blogPostUrl = (locale: 'zh' | 'en', slug: string): string =>
  absoluteUrl(blogPostPath(locale, slug));


/** Site-relative path for router links. */
export const pagePath = (lang: string, page: PageName = 'home'): string =>
  `/${lang}${PAGE_PATHS[page]}`;

export const pageUrl = (lang: string, page: PageName = 'home'): string =>
  absoluteUrl(pagePath(lang, page));

/** The locale set advertised to crawlers. `x-default` is English, the wider market. */
export const alternateLinks = (page: PageName = 'home') => [
  { hrefLang: 'zh-CN', href: pageUrl('zh', page) },
  { hrefLang: 'zh', href: pageUrl('zh', page) },
  { hrefLang: 'en', href: pageUrl('en', page) },
  { hrefLang: 'x-default', href: pageUrl('en', page) },
];

/**
 * Meta descriptions concatenate the two hero subtitle lines, and the Chinese
 * strings already end in `。` — a naive `line1 + '. ' + line2` produced
 * `本地处理。 . 一次买断。`. Join on a single separator and let punctuation stand.
 */
export const joinSentence = (a: string, b: string): string => {
  const left = a.trim();
  const right = b.trim();
  if (!left) return right;
  if (!right) return left;
  const endsWithPunctuation = /[。.!?！？]$/u.test(left);
  return endsWithPunctuation ? `${left} ${right}` : `${left}. ${right}`;
};
