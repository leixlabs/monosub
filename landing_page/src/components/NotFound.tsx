import { Helmet } from 'react-helmet-async';
import { useLanguage } from '../hooks/useLanguage';

/**
 * Rendered both by the CDN (as `dist/404.html`, with a real 404 status) and by
 * the client router for in-app navigations to a URL that has no page.
 *
 * Both languages are shown at once rather than guessing: the visitor here is by
 * definition lost, and a language guess that guesses wrong costs them another
 * click.
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

      <main className="flex min-h-screen flex-col items-center justify-center bg-paper px-6 text-center text-ink-8">
        <p className="eyebrow mb-4">404</p>
        <h1 className="display-2 mb-4 text-ink">
          {isZh ? '这个页面不存在' : "This page doesn't exist"}
        </h1>
        <p className="mb-10 max-w-md leading-relaxed text-ink-6">
          {isZh
            ? '链接可能已经过期，或者地址打错了。'
            : 'The link may be out of date, or the address has a typo.'}
        </p>
        <a href={`/${lang}/`} className="btn btn-primary">
          {isZh ? '返回首页' : 'Back to MonoSub'}
        </a>
      </main>
    </>
  );
};

export default NotFound;
