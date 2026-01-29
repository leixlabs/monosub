export interface LanguageConfig {
  code: string;
  name: string;
  flag: string;
  price: string;
  monthly?: string;
  rtl?: boolean;
}

export interface Translation {
  page_title: string;
  mobile_select_language: string;
  
  // Navigation
  nav_features: string;
  nav_pricing: string;
  nav_faq: string;
  nav_cta: string;
  
  // Hero
  hero_title_line1: string;
  hero_title_line2: string;
  hero_title_line3: string;
  hero_subtitle_line1: string;
  hero_subtitle_line2: string;
  hero_cta: string;
  hero_learn_more: string;
  hero_trust: string;
  
  // Pain Points
  pain_title: string;
  pain_subtitle: string;
  pain_cloud_title: string;
  pain_cloud_1: string;
  pain_cloud_2: string;
  pain_cloud_3: string;
  pain_cloud_4: string;
  pain_local_title: string;
  pain_local_1: string;
  pain_local_2: string;
  pain_local_3: string;
  pain_local_4: string;
  
  // Features
  features_title: string;
  features_subtitle: string;
  feature_1_title: string;
  feature_1_desc: string;
  feature_2_title: string;
  feature_2_desc: string;
  feature_3_title: string;
  feature_3_desc: string;
  feature_4_title: string;
  feature_4_desc: string;
  feature_5_title: string;
  feature_5_desc: string;
  feature_6_title: string;
  feature_6_desc: string;
  
  // Pricing
  pricing_title: string;
  pricing_subtitle: string;
  pricing_period: string;
  pricing_cloud: string;
  pricing_monosub: string;
  pricing_1month: string;
  pricing_6months: string;
  pricing_1year: string;
  pricing_2years: string;
  pricing_5years: string;
  pricing_cloud_1month: string;
  pricing_cloud_6months: string;
  pricing_cloud_1year: string;
  pricing_cloud_2years: string;
  pricing_cloud_5years: string;
  pricing_monosub_price: string;
  pricing_cta: string;
  pricing_save: string;
  
  // Tech
  tech_title: string;
  tech_subtitle: string;
  tech_model: string;
  tech_model_value: string;
  tech_framework: string;
  tech_framework_value: string;
  tech_acceleration: string;
  tech_acceleration_value: string;
  tech_platforms: string;
  tech_platform_macos: string;
  tech_platform_windows: string;
  tech_platform_linux: string;
  
  // FAQ
  faq_title: string;
  faq_subtitle: string;
  faq_1_q: string;
  faq_1_a: string;
  faq_2_q: string;
  faq_2_a: string;
  faq_3_q: string;
  faq_3_a: string;
  faq_4_q: string;
  faq_4_a: string;
  faq_5_q: string;
  faq_5_a: string;
  faq_6_q: string;
  faq_6_a: string;
  
  // Final CTA
  final_title: string;
  final_subtitle: string;
  final_cta: string;
  final_trust_1: string;
  final_trust_2: string;
  final_trust_3: string;
  final_payment: string;
  
  // Footer
  footer_copyright: string;
  footer_terms: string;
  footer_privacy: string;
  footer_contact: string;
}

import zhCN from './locales/zh-CN';
import en from './locales/en';
import es from './locales/es';
import ru from './locales/ru';
import ptBR from './locales/pt-BR';
import ja from './locales/ja';
import ko from './locales/ko';
import de from './locales/de';
import fr from './locales/fr';
import zhTW from './locales/zh-TW';
import es419 from './locales/es-419';
import id from './locales/id';
import cs from './locales/cs';
import it from './locales/it';
import nl from './locales/nl';
import ptPT from './locales/pt-PT';
import pl from './locales/pl';
import vi from './locales/vi';
import tr from './locales/tr';
import uk from './locales/uk';
import ar from './locales/ar';
import fa from './locales/fa';
import th from './locales/th';
import hi from './locales/hi';

export const languageConfigs: LanguageConfig[] = [
  { code: 'zh', name: '中文（简体）', flag: '🇨🇳', price: '¥299', monthly: '¥149' },
  { code: 'en', name: 'English', flag: '🇺🇸', price: '$39', monthly: '$19' },
  { code: 'es', name: 'Español', flag: '🇪🇸', price: '€35', monthly: '€15' },
  { code: 'ru', name: 'Русский', flag: '🇷🇺', price: '₽3,500', monthly: '₽1,500' },
  { code: 'pt-br', name: 'Português (BR)', flag: '🇧🇷', price: 'R$ 199', monthly: 'R$ 89' },
  { code: 'ja', name: '日本語', flag: '🇯🇵', price: '¥5,900', monthly: '¥2,900' },
  { code: 'ko', name: '한국어', flag: '🇰🇷', price: '₩59,000', monthly: '₩29,000' },
  { code: 'de', name: 'Deutsch', flag: '🇩🇪', price: '€35', monthly: '€15' },
  { code: 'fr', name: 'Français', flag: '🇫🇷', price: '€35', monthly: '€15' },
  { code: 'zh-tw', name: '繁體中文', flag: '🇹🇼', price: 'NT$ 1,299', monthly: 'NT$ 599' },
  { code: 'es-419', name: 'Español (LATAM)', flag: '🇲🇽', price: '$39', monthly: '$19' },
  { code: 'id', name: 'Bahasa Indonesia', flag: '🇮🇩', price: 'Rp 599,000', monthly: 'Rp 299,000' },
  { code: 'cs', name: 'Čeština', flag: '🇨🇿', price: 'Kč 899', monthly: 'Kč 449' },
  { code: 'it', name: 'Italiano', flag: '🇮🇹', price: '€35', monthly: '€15' },
  { code: 'nl', name: 'Nederlands', flag: '🇳🇱', price: '€35', monthly: '€15' },
  { code: 'pt-pt', name: 'Português (PT)', flag: '🇵🇹', price: '€35', monthly: '€15' },
  { code: 'pl', name: 'Polski', flag: '🇵🇱', price: 'zł 159', monthly: 'zł 79' },
  { code: 'vi', name: 'Tiếng Việt', flag: '🇻🇳', price: '₫999,000', monthly: '₫499,000' },
  { code: 'tr', name: 'Türkçe', flag: '🇹🇷', price: '₺1,399', monthly: '₺699' },
  { code: 'uk', name: 'Українська', flag: '🇺🇦', price: '₴1,599', monthly: '₴799' },
  { code: 'ar', name: 'العربية', flag: '🇸🇦', price: 'ر.س 149', monthly: 'ر.س 75', rtl: true },
  { code: 'fa', name: 'فارسی', flag: '🇮🇷', price: '﷼1,699,000', monthly: '﷼849,000', rtl: true },
  { code: 'th', name: 'ภาษาไทย', flag: '🇹🇭', price: '฿1,399', monthly: '฿699' },
  { code: 'hi', name: 'हिन्दी', flag: '🇮🇳', price: '₹3,299', monthly: '₹1,599' }
];

export const translations: Record<string, Translation> = {
  'zh': zhCN,
  'en': en,
  'es': es,
  'ru': ru,
  'pt-br': ptBR,
  'ja': ja,
  'ko': ko,
  'de': de,
  'fr': fr,
  'zh-tw': zhTW,
  'es-419': es419,
  'id': id,
  'cs': cs,
  'it': it,
  'nl': nl,
  'pt-pt': ptPT,
  'pl': pl,
  'vi': vi,
  'tr': tr,
  'uk': uk,
  'ar': ar,
  'fa': fa,
  'th': th,
  'hi': hi
};

export const detectBrowserLanguage = (): string => {
  const saved = localStorage.getItem('monosub_language');
  if (saved && translations[saved]) {
    return saved;
  }

  const browserLang = navigator.language || 'en';
  const langMap: Record<string, string> = {
    'zh-CN': 'zh', 'zh-Hans': 'zh', 'zh': 'zh', 'zh-SG': 'zh',
    'zh-TW': 'zh-tw', 'zh-Hant': 'zh-tw', 'zh-HK': 'zh-tw', 'zh-MO': 'zh-tw',
    'en': 'en', 'en-US': 'en', 'en-GB': 'en', 'en-AU': 'en', 'en-CA': 'en',
    'es': 'es', 'es-ES': 'es',
    'es-MX': 'es-419', 'es-AR': 'es-419', 'es-CL': 'es-419', 'es-CO': 'es-419',
    'pt': 'pt-br', 'pt-BR': 'pt-br',
    'pt-PT': 'pt-pt',
    'ja': 'ja', 'ja-JP': 'ja',
    'ko': 'ko', 'ko-KR': 'ko',
    'ru': 'ru', 'ru-RU': 'ru',
    'de': 'de', 'de-DE': 'de', 'de-AT': 'de', 'de-CH': 'de',
    'fr': 'fr', 'fr-FR': 'fr', 'fr-CA': 'fr', 'fr-BE': 'fr',
    'id': 'id', 'id-ID': 'id',
    'cs': 'cs', 'cs-CZ': 'cs',
    'it': 'it', 'it-IT': 'it',
    'nl': 'nl', 'nl-NL': 'nl', 'nl-BE': 'nl',
    'pl': 'pl', 'pl-PL': 'pl',
    'vi': 'vi', 'vi-VN': 'vi',
    'tr': 'tr', 'tr-TR': 'tr',
    'uk': 'uk', 'uk-UA': 'uk',
    'ar': 'ar', 'ar-SA': 'ar', 'ar-EG': 'ar', 'ar-AE': 'ar',
    'fa': 'fa', 'fa-IR': 'fa',
    'th': 'th', 'th-TH': 'th',
    'hi': 'hi', 'hi-IN': 'hi'
  };

  if (langMap[browserLang]) {
    return langMap[browserLang];
  }

  const baseLang = browserLang.split('-')[0];
  return langMap[baseLang] || 'en';
};
