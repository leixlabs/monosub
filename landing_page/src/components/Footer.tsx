import { Link } from 'react-router-dom';
import { useLanguage } from '../hooks/useLanguage';
import { BLOG_PATH, pagePath } from '../lib/seo';

export const Footer = () => {
  const { t, lang } = useLanguage();
  const locale = lang === 'zh' ? 'zh' : 'en';

  return (
    <footer className="border-t border-line bg-paper">
      <div className="container-page py-12 md:py-14">
        <div className="flex flex-col md:flex-row md:items-start md:justify-between gap-8">
          <div className="max-w-sm">
            <p className="font-display text-[1.3rem] font-semibold text-ink">
              MonoSub<span className="text-gold">.</span>
            </p>
            <p className="mt-2 text-sm text-ink-6 leading-relaxed">{t.footer.tagline}</p>
          </div>

          <nav aria-label="Footer" className="flex flex-wrap items-center gap-x-6 gap-y-2 text-sm">
            <Link to={BLOG_PATH[locale]} className="text-ink-7 hover:text-gold no-underline transition-colors">
              {t.footer.blog}
            </Link>
            <Link to={pagePath(lang, 'privacy')} className="text-ink-7 hover:text-gold no-underline transition-colors">
              {t.footer.privacy}
            </Link>
            <a href="mailto:kamalei.zhang@gmail.com" className="text-ink-7 hover:text-gold no-underline transition-colors">
              {t.footer.contact}
            </a>
            <a href="#" className="text-ink-7 hover:text-gold no-underline transition-colors">
              {t.footer.terms}
            </a>
          </nav>
        </div>

        <div className="mt-10 pt-6 border-t border-line/70 flex flex-col sm:flex-row items-start sm:items-center justify-between gap-3">
          <p className="text-xs text-ink-6 m-0">{t.footer.copyright}</p>
          <p className="text-xs text-ink-5 m-0">Local-first. No uploads.</p>
        </div>
      </div>
    </footer>
  );
};
