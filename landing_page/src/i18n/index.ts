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
  pricing_x: string;
  pricing_check: string;
  pricing_feature_transcription: string;
  pricing_feature_export: string;
  pricing_feature_batch: string;
  pricing_feature_speaker: string;
  pricing_feature_support: string;
  pricing_feature: string;
  pricing_free: string;
  pricing_pro: string;
  pricing_pro_price: string;
  pricing_pro_description: string;
  pricing_pro_discount: string;
  pricing_pro_cta: string;
  
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

// Only two locales ship: Simplified Chinese and English. `/zh/` keeps the
// historic short path, `/en/` is the default and therefore the fallback for
// every unlisted language.
import zhCN from './locales/zh-CN';
import en from './locales/en';

export const languageConfigs: LanguageConfig[] = [
  { code: 'zh', name: '中文（简体）', flag: '🇨🇳', price: '¥299', monthly: '¥149' },
  { code: 'en', name: 'English', flag: '🇺🇸', price: '$39', monthly: '$19' }
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
