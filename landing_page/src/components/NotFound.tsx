import { Helmet } from 'react-helmet-async';
import { useLanguage } from '../hooks/useLanguage';

/**
 * Rendered both by the CDN (as `dist/404.html`, with a real 404 status) and by
 * the client router for in-app navigations to a URL that has no page.
 *
 * Both languages are shown at once rather than guessing: the visitor here is by
 * definition lost, and a language guess that guesses wrong costs them another
 * click. The styling is inline because this page must stand on its own — the
 * Tailwind bundle is loaded, but a 404 should not depend on it.
 */
export const NotFound = () => {
  const { lang } = useLanguage();
  const isZh = lang === 'zh';

  const title = isZh ? '页面不存在 — MonoSub' : 'Page not found — MonoSub';

  return (
    <>
      <Helmet>
        <title>{title}</title>
        <meta name="robots" content="noindex, follow" />
      </Helmet>

      <main className="min-h-screen bg-dark-bg text-white flex flex-col items-center justify-center px-6 text-center">
        <p className="text-primary font-semibold tracking-widest text-sm mb-4">404</p>
        <h1 className="text-4xl md:text-5xl font-bold mb-4">
          {isZh ? '这个页面不存在' : "This page doesn't exist"}
        </h1>
        <p className="text-gray-400 mb-10 max-w-md leading-relaxed">
          {isZh
            ? '链接可能已经过期，或者地址打错了。'
            : 'The link may be out of date, or the address has a typo.'}
        </p>
        <a
          href={`/${lang}/`}
          className="cta-button rounded-xl px-8 py-3 font-semibold text-white"
        >
          {isZh ? '返回首页' : 'Back to MonoSub'}
        </a>
      </main>
    </>
  );
};

export default NotFound;
