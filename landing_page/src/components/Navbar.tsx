import { useState, useEffect } from 'react';
import { Link, useLocation } from 'react-router-dom';
import { useLanguage } from '../hooks/useLanguage';
import { LanguageSwitcher } from './LanguageSwitcher';
import { STORE_URL } from '../lib/links';
import { BLOG_PATH, pagePath } from '../lib/seo';

export const Navbar = () => {
  const { t, lang } = useLanguage();
  const { pathname } = useLocation();
  const [scrolled, setScrolled] = useState(false);

  useEffect(() => {
    const handleScroll = () => {
      setScrolled(window.scrollY > 50);
    };

    window.addEventListener('scroll', handleScroll);
    return () => window.removeEventListener('scroll', handleScroll);
  }, []);

  /**
   * The section links are in-page anchors, so they only mean anything on the
   * landing page. On `/blog` they have to point back at the home page's anchors
   * — a bare `#pricing` from a blog post would go nowhere.
   */
  const home = pagePath(lang);
  const isHome = pathname === home || pathname === `${home}/`;
  const sectionLink = (hash: string) => (isHome ? `#${hash}` : `${home}#${hash}`);

  return (
    <nav
      className={`fixed top-0 left-0 right-0 z-50 px-6 py-4 transition-all ${
        scrolled ? 'bg-dark-bg/80 backdrop-blur-lg border-b border-dark-border' : 'bg-transparent'
      }`}
    >
      <div className="max-w-7xl mx-auto flex items-center justify-between">
        <Link to={home} className="text-2xl font-bold gradient-text">
          MonoSub
        </Link>

        {/* Desktop Navigation */}
        <div className="hidden lg:flex items-center gap-6 xl:gap-8">
          <a href={sectionLink('features')} className="text-gray-300 hover:text-white transition-colors">
            {t.nav_features}
          </a>
          <a href={sectionLink('screenshots')} className="text-gray-300 hover:text-white transition-colors">
            {t.nav_screenshots}
          </a>
          <a href={sectionLink('pricing')} className="text-gray-300 hover:text-white transition-colors">
            {t.nav_pricing}
          </a>
          <a href={sectionLink('faq')} className="text-gray-300 hover:text-white transition-colors">
            {t.nav_faq}
          </a>
          <Link
            to={BLOG_PATH[lang === 'zh' ? 'zh' : 'en']}
            className={`transition-colors ${
              pathname.includes('/blog') ? 'text-primary' : 'text-gray-300 hover:text-white'
            }`}
          >
            {t.nav_blog}
          </Link>
          <a
            href={STORE_URL}
            target="_blank"
            rel="noopener noreferrer"
            className="cta-button text-white text-sm font-semibold px-4 py-2 rounded-lg whitespace-nowrap transition-all"
          >
            {t.nav_cta}
          </a>
          <LanguageSwitcher />
        </div>

        {/* Below lg there is no room for the section links, but the way to buy
            and the comparison series must stay reachable on a phone. */}
        <div className="lg:hidden flex items-center gap-2">
          <Link
            to={BLOG_PATH[lang === 'zh' ? 'zh' : 'en']}
            className={`text-sm px-2 ${
              pathname.includes('/blog') ? 'text-primary' : 'text-gray-300'
            }`}
          >
            {t.nav_blog}
          </Link>
          <a
            href={STORE_URL}
            target="_blank"
            rel="noopener noreferrer"
            className="cta-button text-white text-sm font-semibold px-3 py-2 rounded-lg whitespace-nowrap"
          >
            {t.nav_cta}
          </a>
          <LanguageSwitcher />
        </div>
      </div>
    </nav>
  );
};
