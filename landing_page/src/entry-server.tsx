/**
 * Build-time entry, used only by `scripts/prerender.mjs`.
 *
 * React Router runs in memory here (`StaticRouter` + `useRoutes`) instead of the
 * browser router, so the same route tree renders once per URL on the server and
 * the result is written as static HTML. Nothing in this file ships to visitors.
 *
 * The page list lives here so the HTML output, the sitemap and `_redirects` are
 * all derived from one source — a page added to the router is either prerendered
 * or consciously not, never silently missing from the sitemap.
 */

import { renderToString } from 'react-dom/server';
import { StaticRouter } from 'react-router-dom/server';
import { useRoutes } from 'react-router-dom';
import { HelmetProvider, type HelmetServerState } from 'react-helmet-async';
import { createAppRoutes } from './routes';
import { translations } from './i18n';
import { alternateLinks, blogIndexPath, blogPostPath, pagePath, pageUrl } from './lib/seo';
import { POSTS } from './content/blog';
import { SITE_URL, absoluteUrl } from './lib/site';

export { SITE_URL };

/** One entry per URL that gets its own static file. */
export const PRERENDER_PAGES: { url: string; outFile: string }[] = [
  // The bare domain renders the English page: a crawler that lands on `/` must
  // find content, and its canonical points at `/en/` so the two do not compete.
  { url: '/', outFile: 'index.html' },
  ...Object.keys(translations).flatMap((lang) => [
    { url: pagePath(lang, 'home'), outFile: `${lang}/index.html` },
    { url: pagePath(lang, 'privacy'), outFile: `${lang}/privacy/index.html` },
    // The blog index and every post: article bodies are precisely what a crawler
    // that does not execute JS would otherwise never see, so they must be static.
    { url: blogIndexPath(lang as 'zh' | 'en'), outFile: `${lang}/blog/index.html` },
    ...POSTS.map((post) => ({
      url: blogPostPath(lang as 'zh' | 'en', post.slug),
      outFile: `${lang}/blog/${post.slug}/index.html`,
    })),
  ]),
];

/**
 * Indexed URLs for the sitemap — the product pages, the blog index, and each
 * post. `/privacy` is noindex, so it is excluded.
 *
 * Blog URLs carry no hreflang alternates: the two language versions are separate
 * articles, not translations of one page, and claiming otherwise would ask a
 * crawler to treat them as duplicates. Only the index is a true pair.
 */
export const SITEMAP_PAGES = [
  ...Object.keys(translations).map((lang) => ({
    loc: pageUrl(lang, 'home'),
    xDefault: pageUrl('en', 'home'),
    alternates: alternateLinks('home'),
  })),
  ...(['zh', 'en'] as const).flatMap((locale) => [
    {
      loc: absoluteUrl(blogIndexPath(locale)),
      xDefault: absoluteUrl(blogIndexPath('en')),
      alternates: [
        { hrefLang: 'zh-CN', href: absoluteUrl(blogIndexPath('zh')) },
        { hrefLang: 'zh', href: absoluteUrl(blogIndexPath('zh')) },
        { hrefLang: 'en', href: absoluteUrl(blogIndexPath('en')) },
      ],
    },
    ...POSTS.map((post) => ({
      loc: absoluteUrl(blogPostPath(locale, post.slug)),
      xDefault: absoluteUrl(blogPostPath('en', post.slug)),
      alternates: [],
    })),
  ]),
];

/**
 * Locales that shipped in an earlier release and are still in the index. They
 * have no page any more, so without an explicit redirect they would 404 and
 * throw away whatever inbound links they collected.
 */
export const LEGACY_LOCALES = [
  'ar',
  'cs',
  'de',
  'es',
  'fa',
  'fr',
  'hi',
  'id',
  'it',
  'ja',
  'ko',
  'nl',
  'pl',
  'pt-br',
  'pt-pt',
  'ru',
  'th',
  'tr',
  'uk',
  'vi',
];

const AppRoutes = () => useRoutes(createAppRoutes());

export type Rendered = {
  /** Markup for `<div id="root">`. */
  body: string;
  /** Everything Helmet collected, ready to place before `</head>`. */
  head: string;
  /** Attributes for `<html>`, already serialised (`lang="zh" dir="ltr"`). */
  htmlAttributes: string;
};

/**
 * Helmet serialises the props it was given, so JSX's `hrefLang` reaches the HTML
 * as `hrefLang`. Attribute names are case-insensitive to an HTML parser, but the
 * raw file — which is what non-browser consumers (SEO tools, some crawlers, and
 * anyone reading `view-source`) actually see — should say `hreflang`. Normalise
 * the handful of React-style attribute names Helmet can emit.
 */
const HTML_ATTRIBUTE_NAMES: Record<string, string> = {
  hrefLang: 'hreflang',
  charSet: 'charset',
  httpEquiv: 'http-equiv',
  itemProp: 'itemprop',
};

const normaliseAttributeNames = (head: string): string =>
  head.replace(
    /\b(hrefLang|charSet|httpEquiv|itemProp)=/g,
    (_match, name: string) => `${HTML_ATTRIBUTE_NAMES[name]}=`
  );

export const render = (url: string): Rendered => {
  // Helmet fills this object during render; typed as the provider expects.
  const helmetContext: { helmet?: HelmetServerState } = {};

  const body = renderToString(
    <HelmetProvider context={helmetContext}>
      <StaticRouter location={url}>
        <AppRoutes />
      </StaticRouter>
    </HelmetProvider>
  );

  // HelmetProvider always populates `helmet` when rendering on the server.
  const helmet = helmetContext.helmet!;

  // Order matters: title and meta first, then links (canonical, hreflang, icons),
  // then the JSON-LD script Helmet hoisted out of the component tree.
  const head = normaliseAttributeNames(
    [
      helmet.title.toString(),
      helmet.meta.toString(),
      helmet.link.toString(),
      helmet.script.toString(),
    ]
      .filter(Boolean)
      .join('\n    ')
  );

  return {
    body,
    head,
    htmlAttributes: helmet.htmlAttributes.toString(),
  };
};
