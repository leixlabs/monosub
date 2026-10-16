import i18n from 'i18next';
import { initReactI18next } from 'react-i18next';
import LanguageDetector from 'i18next-browser-languagedetector';

// Only two UI locales ship: Simplified Chinese and English. Everything else
// falls back to English (see `convertDetectedLanguage` below).
import en from './locales/en.json';
import zhCN from './locales/zh-CN.json';

const resources = {
  en: { translation: en },
  'zh-CN': { translation: zhCN },
};

export const supportedLanguages = [
  { code: 'zh-CN', flag: '🇨🇳', name: '中文(简体)' },
  { code: 'en', flag: '🇺🇸', name: 'English' },
];

export const UI_LANGUAGE_STORAGE_KEY = 'monosub-language';

/**
 * Collapse any detected language tag onto one of the two shipped locales.
 *
 * `supportedLngs` alone is not enough: i18next resolves both `zh-TW` *and* `zh`
 * down to the default language (`en`) instead of `zh-CN`, and a stale persisted
 * choice (e.g. `zh-TW`, `de`) would silently pin the UI to English. Normalising
 * inside the detector also repairs the cached value.
 */
export const toSupportedLanguage = (language: string): string =>
  language.toLowerCase().startsWith('zh') ? 'zh-CN' : 'en';

i18n
  .use(LanguageDetector)
  .use(initReactI18next)
  .init({
    resources,
    fallbackLng: 'en',
    supportedLngs: supportedLanguages.map((l) => l.code),

    detection: {
      order: ['localStorage', 'navigator', 'htmlTag'],
      lookupLocalStorage: UI_LANGUAGE_STORAGE_KEY,
      caches: ['localStorage'],
      convertDetectedLanguage: toSupportedLanguage,
    },

    interpolation: {
      escapeValue: false,
    },

    react: {
      useSuspense: false,
    },
  });

export default i18n;
