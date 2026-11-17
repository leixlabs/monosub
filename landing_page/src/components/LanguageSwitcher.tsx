import { useLocation, useNavigate } from 'react-router-dom';
import { useLanguage } from '../hooks/useLanguage';
import { BLOG_PATH } from '../lib/seo';

/**
 * Language switch — no flags. The button always names the language you would
 * switch TO: the Chinese page shows “English”, the English page shows “中文”.
 * With only two locales this is a single toggle, so there is no menu to open.
 */
const SHORT_LABEL: Record<string, string> = {
  zh: '中文',
  en: 'English',
};

export const LanguageSwitcher = () => {
  const navigate = useNavigate();
  const { pathname } = useLocation();
  const { lang } = useLanguage();

  const otherLang = lang === 'zh' ? 'en' : 'zh';

  /** Keep the visitor on the same page: a blog post keeps its slug. */
  const targetFor = (newLang: string): string => {
    const post = pathname.match(/\/(?:zh|en)\/blog\/([^/]+)\/?$/);
    if (post) return `${BLOG_PATH[newLang as 'zh' | 'en']}${post[1]}/`;
    if (pathname.includes('/blog')) return BLOG_PATH[newLang as 'zh' | 'en'];
    if (/\/privacy\/?$/.test(pathname)) return `/${newLang}/privacy`;
    return `/${newLang}`;
  };

  const handleChange = () => {
    navigate(targetFor(otherLang));
    localStorage.setItem('monosub_language', otherLang);
  };

  return (
    <button
      type="button"
      onClick={handleChange}
      lang={otherLang}
      className="flex items-center gap-1.5 min-h-[2.4rem] px-3 rounded border border-line bg-paper text-ink text-sm hover:border-ink-5 hover:text-gold transition-colors"
      aria-label={otherLang === 'zh' ? '切换到中文' : 'Switch to English'}
    >
      <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
        <circle cx="12" cy="12" r="9" />
        <path d="M3 12h18M12 3c2.5 2.7 2.5 15.3 0 18M12 3c-2.5 2.7-2.5 15.3 0 18" />
      </svg>
      <span>{SHORT_LABEL[otherLang]}</span>
    </button>
  );
};
