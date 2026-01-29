import { useParams } from 'react-router-dom';
import { translations, languageConfigs } from '../i18n';
import type { Translation, LanguageConfig } from '../i18n';

export const useLanguage = () => {
  const { lang } = useParams<{ lang: string }>();
  const currentLang = lang || 'en';
  
  const t: Translation = translations[currentLang] || translations['en'];
  const config: LanguageConfig = languageConfigs.find(l => l.code === currentLang) || languageConfigs[1];
  
  return {
    lang: currentLang,
    t,
    config,
    isRTL: config.rtl || false
  };
};
