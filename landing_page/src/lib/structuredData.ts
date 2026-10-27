/**
 * JSON-LD for the landing page.
 *
 * Two graphs, one script tag:
 *
 * - `SoftwareApplication` — the thing being sold. Without it a search engine has
 *   to infer that this page is a product page; with it, the price, the platforms
 *   and the category are explicit and can surface as a rich result.
 * - `FAQPage` — the questions already rendered on the page. Google only treats
 *   FAQ markup as valid when the same Q&A is visible to the visitor, which is
 *   exactly the case here: both graphs are generated from the same translation
 *   strings the components render.
 *
 * Both are built from `t`, so a copy change in `locales/*.ts` cannot leave the
 * markup describing a stale product.
 */

import type { Translation } from '../i18n';
import { faqPairs } from './faq';
import { absoluteUrl } from './site';

type JsonLd = Record<string, unknown>;

/** The Windows price is the paid one; macOS ships from GitHub Releases for free. */
const PRICE = {
  usd: '9.99',
  cny: '48',
} as const;

export const buildStructuredData = (
  lang: string,
  t: Translation,
  pagePath: string
): JsonLd => {
  const isZh = lang === 'zh';
  const pageUrl = absoluteUrl(pagePath);
  const description = `${t.hero_subtitle_line1} ${t.hero_subtitle_line2}`.trim();

  const app: JsonLd = {
    '@type': 'SoftwareApplication',
    '@id': `${pageUrl}#software`,
    name: 'MonoSub',
    applicationCategory: 'MultimediaApplication',
    applicationSubCategory: isZh ? '字幕生成 / 语音转写' : 'Subtitle generation / speech transcription',
    operatingSystem: 'Windows 10, Windows 11, macOS 11+',
    description,
    url: pageUrl,
    inLanguage: isZh ? 'zh-CN' : 'en',
    image: absoluteUrl('/og-image.jpg'),
    softwareHelp: absoluteUrl(isZh ? '/zh' : '/en'),
    featureList: [
      t.feature_1_title,
      t.feature_2_title,
      t.feature_3_title,
      t.feature_4_title,
      t.feature_5_title,
      t.feature_6_title,
      t.feature_7_title,
      t.feature_8_title,
    ]
      // The titles carry an emoji prefix for the UI; strip it for machine data.
      .map((feature) => feature.replace(/^[^\p{L}\p{N}]+/u, '').trim()),
    offers: [
      {
        '@type': 'Offer',
        price: PRICE.usd,
        priceCurrency: 'USD',
        name: 'MonoSub for Windows (Microsoft Store)',
        url: 'https://apps.microsoft.com/detail/9P6WTCCRQTVW',
        availability: 'https://schema.org/InStock',
        priceValidUntil: '2027-12-31',
      },
      {
        '@type': 'Offer',
        price: PRICE.cny,
        priceCurrency: 'CNY',
        name: isZh ? 'MonoSub Windows 版（微软商店）' : 'MonoSub for Windows (Microsoft Store)',
        url: 'https://apps.microsoft.com/detail/9P6WTCCRQTVW',
        availability: 'https://schema.org/InStock',
        priceValidUntil: '2027-12-31',
      },
    ],
    sameAs: ['https://github.com/leixlabs/monosub'],
  };

  const faq: JsonLd = {
    '@type': 'FAQPage',
    '@id': `${pageUrl}#faq`,
    inLanguage: isZh ? 'zh-CN' : 'en',
    mainEntity: faqPairs(t).map(({ q, a }) => ({
      '@type': 'Question',
      name: q,
      acceptedAnswer: { '@type': 'Answer', text: a },
    })),
  };

  const website: JsonLd = {
    '@type': 'WebSite',
    '@id': `${absoluteUrl('/')}#website`,
    url: absoluteUrl('/'),
    name: 'MonoSub',
    inLanguage: ['zh-CN', 'en'],
  };

  return {
    '@context': 'https://schema.org',
    '@graph': [app, faq, website],
  };
};
