import { useState, useEffect } from 'react';
import { useLanguage } from '../hooks/useLanguage';
import { LanguageSwitcher } from './LanguageSwitcher';

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
        <div className="hidden md:flex items-center gap-8">
          <a href="#features" className="text-gray-300 hover:text-white transition-colors">
            {t.nav_features}
          </a>
          <a href="#pricing" className="text-gray-300 hover:text-white transition-colors">
            {t.nav_pricing}
          </a>
          <a href="#faq" className="text-gray-300 hover:text-white transition-colors">
            {t.nav_faq}
          </a>
          <LanguageSwitcher />
          <a
            href="#cta"
            className="cta-button text-white font-semibold px-6 py-2 rounded-lg transition-all"
          >
            {t.nav_cta}
          </a>
        </div>

        {/* Mobile Navigation */}
        <div className="md:hidden">
          <LanguageSwitcher />
        </div>
      </div>
    </nav>
  );
};
