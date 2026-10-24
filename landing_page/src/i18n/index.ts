export interface LanguageConfig {
  code: string;
  name: string;
  flag: string;
  rtl?: boolean;
}

export interface Translation {
  page_title: string;
  mobile_select_language: string;
  
  // Navigation
  nav_features: string;
  nav_screenshots: string;
  nav_faq: string;

  // Hero
  hero_title_line1: string;
  hero_title_line2: string;
  hero_title_line3: string;
  hero_subtitle_line1: string;
  hero_subtitle_line2: string;
  hero_cta: string;
  hero_learn_more: string;
  hero_trust: string;
  hero_platform: string;

  // Hero carousel
  carousel_prev: string;
  carousel_next: string;
  hero_shot_1: string;
  hero_shot_2: string;
  hero_shot_3: string;
  hero_shot_4: string;
  hero_shot_5: string;

  // Screenshot showcase
  showcase_title: string;
  showcase_subtitle: string;
  showcase_tab_queue: string;
  showcase_tab_translate: string;
  showcase_tab_speakers: string;
  showcase_tab_url: string;
  showcase_queue_desc: string;
  showcase_translate_desc: string;
  showcase_speakers_desc: string;
  showcase_url_desc: string;
  shot_queue: string;
  shot_batch: string;
  shot_rowstates: string;
  shot_translated: string;
  shot_translate_settings: string;
  shot_models: string;
  shot_catalogue: string;
  shot_url_rows: string;
  shot_url_dialog: string;
  shot_clipboard: string;
  
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
  feature_7_title: string;
  feature_7_desc: string;
  feature_8_title: string;
  feature_8_desc: string;
  
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
  
  // Final CTA
  final_title: string;
  final_subtitle: string;
  final_cta: string;
  final_trust_1: string;
  final_trust_2: string;
  final_trust_3: string;
  
  // Footer
  footer_copyright: string;
  footer_terms: string;
  footer_privacy: string;
  footer_contact: string;
}

/** Type-safe lookup for a key that is built at runtime (e.g. a tab id). */
export type TranslationKey = keyof Translation;

// Only two locales ship: Simplified Chinese and English. `/zh/` keeps the
// historic short path, `/en/` is the default and therefore the fallback for
// every unlisted language.
import zhCN from './locales/zh-CN';
import en from './locales/en';

export const languageConfigs: LanguageConfig[] = [
  { code: 'zh', name: '中文（简体）', flag: '🇨🇳' },
  { code: 'en', name: 'English', flag: '🇺🇸' }
];

export const translations: Record<string, Translation> = {
  'zh': zhCN,
  'en': en
};

export const DEFAULT_LANGUAGE = 'en';

export const detectBrowserLanguage = (): string => {
  const saved = localStorage.getItem('monosub_language');
  if (saved && translations[saved]) {
    return saved;
  }

  // Everything that is not Chinese resolves to English.
  const browserLang = (navigator.language || DEFAULT_LANGUAGE).toLowerCase();
  return browserLang.startsWith('zh') ? 'zh' : DEFAULT_LANGUAGE;
};
