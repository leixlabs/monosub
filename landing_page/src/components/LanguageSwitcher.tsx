import { useState } from 'react';
import { useLocation, useNavigate, useParams } from 'react-router-dom';
import { languageConfigs } from '../i18n';
import { useLanguage } from '../hooks/useLanguage';

export const LanguageSwitcher = () => {
  const [isOpen, setIsOpen] = useState(false);
  const navigate = useNavigate();
  const { pathname } = useLocation();
  const { lang } = useParams();
  const { config } = useLanguage();

  const handleLanguageChange = (newLang: string) => {
    // Keep the visitor on the same page (e.g. /privacy) when switching language.
    const target = pathname.endsWith('/privacy')
      ? `/${newLang}/privacy`
      : `/${newLang}`;
    navigate(target);
    setIsOpen(false);
    localStorage.setItem('monosub_language', newLang);
  };

  return (
    <div className="relative">
      <button
        onClick={() => setIsOpen(!isOpen)}
        className="flex items-center gap-2 px-4 py-2 rounded-lg bg-dark-card border border-dark-border hover:border-primary transition-colors"
      >
        <span className="text-xl">{config.flag}</span>
        <span className="hidden sm:inline text-sm">{config.name}</span>
        <svg
          className={`w-4 h-4 transition-transform ${isOpen ? 'rotate-180' : ''}`}
          fill="none"
          stroke="currentColor"
          viewBox="0 0 24 24"
        >
          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 9l-7 7-7-7" />
        </svg>
      </button>

      {isOpen && (
        <>
          <div
            className="fixed inset-0 z-40"
            onClick={() => setIsOpen(false)}
          />
          <div className="absolute right-0 mt-2 w-64 max-h-96 overflow-y-auto bg-dark-card border border-dark-border rounded-lg shadow-xl z-50">
            {languageConfigs.map((langConfig) => (
              <button
                key={langConfig.code}
                onClick={() => handleLanguageChange(langConfig.code)}
                className={`w-full flex items-center gap-3 px-4 py-3 hover:bg-dark-border transition-colors ${
                  lang === langConfig.code ? 'bg-primary/10 text-primary' : 'text-gray-300'
                }`}
              >
                <span className="text-xl">{langConfig.flag}</span>
                <span className="text-sm">{langConfig.name}</span>
                {lang === langConfig.code && (
                  <svg className="w-5 h-5 ml-auto" fill="currentColor" viewBox="0 0 20 20">
                    <path fillRule="evenodd" d="M16.707 5.293a1 1 0 010 1.414l-8 8a1 1 0 01-1.414 0l-4-4a1 1 0 011.414-1.414L8 12.586l7.293-7.293a1 1 0 011.414 0z" clipRule="evenodd" />
                  </svg>
                )}
              </button>
            ))}
          </div>
        </>
      )}
    </div>
  );
};
