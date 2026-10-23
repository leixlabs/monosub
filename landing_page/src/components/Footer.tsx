import { Link } from 'react-router-dom';
import { useLanguage } from '../hooks/useLanguage';

export const Footer = () => {
  const { t, lang } = useLanguage();

  return (
    <footer className="py-12 px-6 border-t border-dark-border">
      <div className="max-w-6xl mx-auto">
        <div className="flex flex-col md:flex-row items-center justify-between gap-6">
          <p className="text-gray-400 text-sm">{t.footer_copyright}</p>
          
          <div className="flex items-center gap-6 text-sm">
            <a href="#" className="text-gray-400 hover:text-primary transition-colors">
              {t.footer_terms}
            </a>
            <Link to={`/${lang}/privacy`} className="text-gray-400 hover:text-primary transition-colors">
              {t.footer_privacy}
            </Link>
            <a href="mailto:kamalei.zhang@gmail.com" className="text-gray-400 hover:text-primary transition-colors">
              {t.footer_contact}
            </a>
          </div>
        </div>
      </div>
    </footer>
  );
};
