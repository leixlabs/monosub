import i18n from 'i18next';
import { initReactI18next } from 'react-i18next';
import LanguageDetector from 'i18next-browser-languagedetector';

// Import all locale files
import en from './locales/en.json';
import zhCN from './locales/zh-CN.json';
import zhTW from './locales/zh-TW.json';
import es from './locales/es.json';
import es419 from './locales/es-419.json';
import ru from './locales/ru.json';
import ptBR from './locales/pt-BR.json';
import ptPT from './locales/pt-PT.json';
import ja from './locales/ja.json';
import ko from './locales/ko.json';
import de from './locales/de.json';
import fr from './locales/fr.json';
import it from './locales/it.json';
import nl from './locales/nl.json';
import pl from './locales/pl.json';
import vi from './locales/vi.json';
import tr from './locales/tr.json';
import uk from './locales/uk.json';
import ar from './locales/ar.json';
import th from './locales/th.json';
import hi from './locales/hi.json';
import id from './locales/id.json';
import cs from './locales/cs.json';
import fa from './locales/fa.json';

const resources = {
  en: { translation: en },
  'zh-CN': { translation: zhCN },
  'zh-TW': { translation: zhTW },
  es: { translation: es },
  'es-419': { translation: es419 },
  ru: { translation: ru },
  'pt-BR': { translation: ptBR },
  'pt-PT': { translation: ptPT },
  ja: { translation: ja },
  ko: { translation: ko },
  de: { translation: de },
  fr: { translation: fr },
  it: { translation: it },
  nl: { translation: nl },
  pl: { translation: pl },
  vi: { translation: vi },
  tr: { translation: tr },
  uk: { translation: uk },
  ar: { translation: ar },
  th: { translation: th },
  hi: { translation: hi },
  id: { translation: id },
  cs: { translation: cs },
  fa: { translation: fa },
};

export const supportedLanguages = [
  { code: 'zh-CN', flag: '🇨🇳', name: '中文(简体)' },
  { code: 'en', flag: '🇺🇸', name: 'English' },
  { code: 'es', flag: '🇪🇸', name: 'Español' },
  { code: 'ru', flag: '🇷🇺', name: 'Русский' },
  { code: 'pt-BR', flag: '🇧🇷', name: 'Português (Brasil)' },
  { code: 'ja', flag: '🇯🇵', name: '日本語' },
  { code: 'ko', flag: '🇰🇷', name: '한국어' },
  { code: 'de', flag: '🇩🇪', name: 'Deutsch' },
  { code: 'fr', flag: '🇫🇷', name: 'Français' },
  { code: 'zh-TW', flag: '🇹🇼', name: '繁體中文' },
  { code: 'es-419', flag: '🇲🇽', name: 'Español (Latinoamérica)' },
  { code: 'id', flag: '🇮🇩', name: 'Bahasa Indonesia' },
  { code: 'cs', flag: '🇨🇿', name: 'Čeština' },
  { code: 'it', flag: '🇮🇹', name: 'Italiano' },
  { code: 'nl', flag: '🇳🇱', name: 'Nederlands' },
  { code: 'pt-PT', flag: '🇵🇹', name: 'Português (Portugal)' },
  { code: 'pl', flag: '🇵🇱', name: 'Polski' },
  { code: 'vi', flag: '🇻🇳', name: 'Tiếng Việt' },
  { code: 'tr', flag: '🇹🇷', name: 'Türkçe' },
  { code: 'uk', flag: '🇺🇦', name: 'Українська' },
  { code: 'ar', flag: '🇸🇦', name: 'العربية' },
  { code: 'fa', flag: '🇮🇷', name: 'فارسی' },
  { code: 'th', flag: '🇹🇭', name: 'ภาษาไทย' },
  { code: 'hi', flag: '🇮🇳', name: 'हिन्दी' },
];

i18n
  .use(LanguageDetector)
  .use(initReactI18next)
  .init({
    resources,
    fallbackLng: 'en',
    supportedLngs: supportedLanguages.map(l => l.code),

    detection: {
      order: ['localStorage', 'navigator', 'htmlTag'],
      lookupLocalStorage: 'monosub-language',
      caches: ['localStorage'],
    },

    interpolation: {
      escapeValue: false,
    },

    react: {
      useSuspense: false,
    },
  });

export default i18n;
