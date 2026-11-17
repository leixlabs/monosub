/**
 * The landing page's copy model.
 *
 * SEO strategy for this page is question-first, not keyword-stuffed: the page is
 * built as a ladder of the real questions someone asks before they install a
 * subtitle tool ("does it upload my files?", "is it free?", "does it run on my
 * Mac?"), and the product is the answer to each. So the shape below mirrors that
 * ladder — one object per question section — instead of a flat list of marketing
 * keys. Structured data (FAQPage / HowTo / SoftwareApplication) is generated from
 * these same objects, which is what keeps the markup and the visible page in
 * agreement.
 *
 * Only two locales ship: Simplified Chinese (`zh`) and English (`en`, default).
 */

export interface LanguageConfig {
  code: string;
  name: string;
  flag: string;
  rtl?: boolean;
}

export interface FaqItem {
  q: string;
  a: string;
}

export interface StatItem {
  value: string;
  label: string;
}

export interface HowStep {
  title: string;
  desc: string;
}

export interface FeatureItem {
  title: string;
  desc: string;
}

export interface ShowcaseTour {
  id: 'queue' | 'translate' | 'speakers' | 'url';
  label: string;
  desc: string;
  /** Captions aligned, index for index, with the shot names in Showcase.tsx. */
  shots: string[];
}

export interface Plan {
  id: 'free' | 'pro';
  name: string;
  badge?: string;
  price: string;
  priceNote: string;
  summary: string;
  features: string[];
  cta: string;
}

export type ChannelId = 'appstore' | 'direct' | 'windows' | 'source';

export interface Channel {
  id: ChannelId;
  platform: string;
  price: string;
  pro: string;
  note: string;
}

export interface Translation {
  lang_name: string;

  // — document head / structured data —
  page_title: string;
  page_description: string;
  og_alt: string;

  nav: {
    features: string;
    how: string;
    pricing: string;
    faq: string;
    blog: string;
    cta_primary: string;
    cta_secondary: string;
  };

  hero: {
    eyebrow: string;
    title: string;
    title_accent: string;
    subtitle: string;
    cta_primary: string;
    cta_secondary: string;
    note: string;
    trust: string[];
    shot_caption: string;
  };

  answer: {
    eyebrow: string;
    title: string;
    body: string;
    stats: StatItem[];
  };

  why: {
    eyebrow: string;
    q: string;
    a: string;
    cloud_title: string;
    local_title: string;
    cloud: string[];
    local: string[];
  };

  how: {
    eyebrow: string;
    title: string;
    subtitle: string;
    steps: HowStep[];
    note: string;
  };

  showcase: {
    eyebrow: string;
    title: string;
    subtitle: string;
    tours: ShowcaseTour[];
  };

  features: {
    eyebrow: string;
    title: string;
    subtitle: string;
    items: FeatureItem[];
  };

  pricing: {
    eyebrow: string;
    title: string;
    subtitle: string;
    plans: Plan[];
    proNoteTitle: string;
    proNote: string;
    channelsTitle: string;
    channelsNote: string;
    columns: { platform: string; price: string; pro: string };
    channels: Channel[];
  };

  faq: {
    eyebrow: string;
    title: string;
    subtitle: string;
    items: FaqItem[];
  };

  final: {
    title: string;
    subtitle: string;
    cta_primary: string;
    cta_secondary: string;
    trust: string[];
  };

  footer: {
    tagline: string;
    blog: string;
    privacy: string;
    contact: string;
    terms: string;
    copyright: string;
  };
}

import zhCN from './locales/zh-CN';
import en from './locales/en';

export const languageConfigs: LanguageConfig[] = [
  { code: 'zh', name: '中文（简体）', flag: '🇨🇳' },
  { code: 'en', name: 'English', flag: '🇺🇸' },
];

export const translations: Record<string, Translation> = {
  zh: zhCN,
  en,
};

export const DEFAULT_LANGUAGE = 'en';

/**
 * Browser-only by nature (it reads `localStorage` and `navigator`), but the same
 * route tree is evaluated during prerendering, where neither exists — so the
 * server path answers with the default locale instead of throwing.
 */
export const detectBrowserLanguage = (): string => {
  if (typeof window === 'undefined') return DEFAULT_LANGUAGE;

  const saved = localStorage.getItem('monosub_language');
  if (saved && translations[saved]) {
    return saved;
  }

  const browserLang = (navigator.language || DEFAULT_LANGUAGE).toLowerCase();
  return browserLang.startsWith('zh') ? 'zh' : DEFAULT_LANGUAGE;
};
