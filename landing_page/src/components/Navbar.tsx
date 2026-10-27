import { useState, useEffect } from 'react';
import { useLanguage } from '../hooks/useLanguage';
import { LanguageSwitcher } from './LanguageSwitcher';
import { STORE_URL } from '../lib/links';

export const Navbar = () => {
  const { t } = useLanguage();
  const [scrolled, setScrolled] = useState(false);

  useEffect(() => {
    const handleScroll = () => {
      setScrolled(window.scrollY > 50);
    };

    window.addEventListener('scroll', handleScroll);
    return () => window.removeEventListener('scroll', handleScroll);
  }, []);

  return (
    <nav
      className={`fixed top-0 left-0 right-0 z-50 px-6 py-4 transition-all ${
        scrolled ? 'bg-dark-bg/80 backdrop-blur-lg border-b border-dark-border' : 'bg-transparent'
      }`}
    >
      <div className="max-w-7xl mx-auto flex items-center justify-between">
        <div className="text-2xl font-bold gradient-text">MonoSub</div>

        {/* Desktop Navigation */}
        <div className="hidden lg:flex items-center gap-6 xl:gap-8">
          <a href="#features" className="text-gray-300 hover:text-white transition-colors">
            {t.nav_features}
          </a>
          <a href="#screenshots" className="text-gray-300 hover:text-white transition-colors">
            {t.nav_screenshots}
          </a>
          <a href="#pricing" className="text-gray-300 hover:text-white transition-colors">
            {t.nav_pricing}
          </a>
          <a href="#faq" className="text-gray-300 hover:text-white transition-colors">
            {t.nav_faq}
          </a>
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

        {/* Below lg there is no room for the links, but the way to buy must stay */}
        <div className="lg:hidden flex items-center gap-2">
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
