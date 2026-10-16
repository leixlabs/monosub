import { useParams } from 'react-router-dom';
import { translations, languageConfigs, DEFAULT_LANGUAGE } from '../i18n';
import type { Translation, LanguageConfig } from '../i18n';

export const useLanguage = () => {
  const { lang } = useParams<{ lang: string }>();
  const currentLang = lang || DEFAULT_LANGUAGE;
  
  const t: Translation = translations[currentLang] || translations[DEFAULT_LANGUAGE];
  const config: LanguageConfig =
    languageConfigs.find(l => l.code === currentLang)
    || languageConfigs.find(l => l.code === DEFAULT_LANGUAGE)!;
  
  return {
    lang: currentLang,
    t,
    config,
    isRTL: config.rtl || false
  };
};
