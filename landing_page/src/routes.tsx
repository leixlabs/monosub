import type { ReactNode } from 'react';
import { Navigate, useParams } from 'react-router-dom';
import App from './App';
import PrivacyPolicy from './components/PrivacyPolicy';
import NotFound from './components/NotFound';
import BlogIndex from './components/BlogIndex';
import BlogPost from './components/BlogPost';
import { DEFAULT_LANGUAGE, translations } from './i18n';
import { pagePath } from './lib/seo';

const validLangs = Object.keys(translations);

/**
 * Old hreflang URLs (`/zh-tw/`, `/ja/`, …) are still indexed; fold them into a
 * language that still exists rather than 404-ing them. `dist/_redirects` catches
 * these at the edge; this is the client-side belt to that suspenders, so an
 * in-app navigation to a dead locale still lands somewhere sensible.
 */
const resolveLanguage = (lang: string): string => {
  const lower = lang.toLowerCase();
  if (lower === 'zh' || lower.startsWith('zh-')) return 'zh';
  return DEFAULT_LANGUAGE;
};

/**
 * Guard for the `/:lang` segment.
 *
 * Written as a redirect *element* rather than a route `loader` on purpose: these
 * routes are evaluated twice — once by `StaticRouter` during prerendering, once
 * by the browser — and loaders only run in the browser. An element-based
 * redirect produces identical output in both passes, which is what hydration
 * needs.
 */
const LangGate = ({ children }: { children: ReactNode }) => {
  const { lang } = useParams<{ lang: string }>();
  if (!lang || !validLangs.includes(lang)) {
    return <Navigate to={pagePath(resolveLanguage(lang ?? ''))} replace />;
  }
  return <>{children}</>;
};

/**
 * `/blog/<slug>` (no locale) → `/en/blog/<slug>/`. Reads the slug out of the
 * route params; interpolating the literal `:slug` into the target string would
 * be a 404 for every reader who follows such a link.
 */
const BlogSlugRedirect = () => {
  const { slug } = useParams<{ slug: string }>();
  return <Navigate to={`/${DEFAULT_LANGUAGE}/blog/${slug ?? ''}/`} replace />;
};

/**
 * One route tree, two consumers: the browser router and the prerenderer. Sharing
 * it is what keeps the static HTML and the hydrated app describing the same
 * URLs — a second copy of this list would drift the moment a page is added.
 *
 * Route order note: `/:lang/` is `end: true`, so it does not swallow `/:lang/blog/`.
 * The bare `/blog` routes exist so that a link missing its locale lands on the
 * English index instead of being read as a language code named "blog".
 */
export const createAppRoutes = () => [
  {
    // The bare domain is the English page. It is *not* a redirect: a crawler
    // landing on `/` must find real content, so the English page renders here
    // with its canonical pointing at `/en/` to keep the two from competing. The
    // language switch for humans is the inline script in `index.html`, which
    // runs before paint.
    path: '/',
    element: <App />,
  },
  {
    // No page of its own; the store listing and older links still point here.
    path: '/privacy',
    element: <Navigate to={pagePath(DEFAULT_LANGUAGE, 'privacy')} replace />,
  },
  {
    // A locale-less blog link lands on the English index. `:slug` has to be read
    // from the params — writing it into the target string as a literal would send
    // `/blog/macwhisper` to `/en/blog/:slug` and 404 the reader.
    path: '/blog',
    element: <Navigate to={`/${DEFAULT_LANGUAGE}/blog/`} replace />,
  },
  {
    path: '/blog/:slug',
    element: <BlogSlugRedirect />,
  },
  {
    path: '/:lang/',
    element: (
      <LangGate>
        <App />
      </LangGate>
    ),
  },
  {
    path: '/:lang/privacy/',
    element: (
      <LangGate>
        <PrivacyPolicy />
      </LangGate>
    ),
  },
  {
    path: '/:lang/blog/',
    element: (
      <LangGate>
        <BlogIndex />
      </LangGate>
    ),
  },
  {
    path: '/:lang/blog/:slug/',
    element: (
      <LangGate>
        <BlogPost />
      </LangGate>
    ),
  },
  {
    // The CDN answers unknown paths with a real 404 (`dist/404.html`), so this
    // only renders for client-side navigations that never hit the server. It
    // must not redirect: a redirect here is exactly the soft-404 that let junk
    // URLs look valid to crawlers.
    path: '*',
    element: <NotFound />,
  },
];
