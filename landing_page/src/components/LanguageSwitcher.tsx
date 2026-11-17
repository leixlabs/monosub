import { useEffect, useRef, useState } from 'react';
import { useLocation, useNavigate, useParams } from 'react-router-dom';
import { languageConfigs } from '../i18n';
import { useLanguage } from '../hooks/useLanguage';
import { BLOG_PATH } from '../lib/seo';

export const LanguageSwitcher = () => {
  const [isOpen, setIsOpen] = useState(false);
  const navigate = useNavigate();
  const { pathname } = useLocation();
  const { lang } = useParams();
  const { config } = useLanguage();
  const ref = useRef<HTMLDivElement>(null);
  const triggerRef = useRef<HTMLButtonElement>(null);

  /**
   * Keep the visitor on the same page when switching language: privacy, the blog
   * index, and a blog post (where the slug must survive) each need their own
   * target. Everything else is the landing page.
   */
  const targetFor = (newLang: string): string => {
    const post = pathname.match(/\/(?:zh|en)\/blog\/([^/]+)\/?$/);
    if (post) return `${BLOG_PATH[newLang as 'zh' | 'en']}${post[1]}/`;
    if (pathname.includes('/blog')) return BLOG_PATH[newLang as 'zh' | 'en'];
    if (/\/privacy\/?$/.test(pathname)) return `/${newLang}/privacy`;
    return `/${newLang}`;
  };

  const handleLanguageChange = (newLang: string) => {
    navigate(targetFor(newLang));
    setIsOpen(false);
    localStorage.setItem('monosub_language', newLang);
    triggerRef.current?.focus();
  };

  // Light dismiss: click outside or Escape closes the popup.
  useEffect(() => {
    if (!isOpen) return;
    const onPointer = (event: PointerEvent) => {
      if (ref.current && !ref.current.contains(event.target as Node)) setIsOpen(false);
    };
    const onKey = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        setIsOpen(false);
        triggerRef.current?.focus();
      }
    };
    document.addEventListener('pointerdown', onPointer);
    document.addEventListener('keydown', onKey);
    return () => {
      document.removeEventListener('pointerdown', onPointer);
      document.removeEventListener('keydown', onKey);
    };
  }, [isOpen]);

  return (
    <div className="relative" ref={ref}>
      <button
        ref={triggerRef}
        type="button"
        onClick={() => setIsOpen((v) => !v)}
        className="flex items-center gap-2 min-h-[2.4rem] px-3 rounded border border-line bg-paper text-ink text-sm hover:border-ink-5 transition-colors"
        aria-haspopup="menu"
        aria-expanded={isOpen}
        aria-label={config.name}
      >
        <span aria-hidden="true" className="text-base">{config.flag}</span>
        <span className="hidden xl:inline">{config.name}</span>
        <svg
          className={`w-3.5 h-3.5 transition-transform ${isOpen ? 'rotate-180' : ''}`}
          fill="none"
          stroke="currentColor"
          viewBox="0 0 24 24"
          aria-hidden="true"
        >
          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 9l-7 7-7-7" />
        </svg>
      </button>

      {isOpen && (
        <div
          role="menu"
          className="absolute right-0 mt-2 w-48 overflow-y-auto card bg-paper p-1 z-50"
        >
          {languageConfigs.map((langConfig) => (
            <button
              key={langConfig.code}
              type="button"
              role="menuitemradio"
              aria-checked={lang === langConfig.code}
              onClick={() => handleLanguageChange(langConfig.code)}
              className={`w-full flex items-center gap-3 px-3 py-2 rounded text-sm text-left transition-colors ${
                lang === langConfig.code ? 'bg-paper-2 text-gold font-semibold' : 'text-ink-7 hover:bg-paper-2'
              }`}
            >
              <span className="text-base" aria-hidden="true">{langConfig.flag}</span>
              <span>{langConfig.name}</span>
            </button>
          ))}
        </div>
      )}
    </div>
  );
};
