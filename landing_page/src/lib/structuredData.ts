/**
 * JSON-LD for the landing page.
 *
 * Four graphs, one script tag, all generated from the same translation objects
 * the components render — so a copy change can never leave the markup describing
 * a stale product:
 *
 * - `SoftwareApplication` — what the page sells. Now that the app is free with a
 *   one-time Pro in-app purchase, the offers list the $0 download *and* the Pro
 *   unlock, instead of a single paid SKU.
 * - `FAQPage` — the exact Q&A rendered in the FAQ section.
 * - `HowTo` — the three local steps rendered in "How it works".
 * - `WebSite` — the site itself, for sitelinks.
 *
 * FAQ and HowTo markup are only valid when the same content is visible to the
 * visitor; both are, because the components read the same arrays.
 */

import type { Translation } from '../i18n';
import { absoluteUrl } from './site';
import { APP_STORE_URL, MS_STORE_URL, CHECKOUT_URL, REPO_URL } from './links';

type JsonLd = Record<string, unknown>;

export const buildStructuredData = (
  lang: string,
  t: Translation,
  pagePath: string
): JsonLd => {
  const isZh = lang === 'zh';
  const pageUrl = absoluteUrl(pagePath);

  const app: JsonLd = {
    '@type': 'SoftwareApplication',
    '@id': `${pageUrl}#software`,
    name: 'MonoSub',
    applicationCategory: 'MultimediaApplication',
    applicationSubCategory: isZh ? '字幕生成 / 语音转写' : 'Subtitle generation / speech transcription',
    operatingSystem: 'macOS 12+, Windows 10, Windows 11',
    description: t.page_description,
    url: pageUrl,
    downloadUrl: APP_STORE_URL,
    installUrl: APP_STORE_URL,
    inLanguage: isZh ? 'zh-CN' : 'en',
    image: absoluteUrl('/og-image.jpg'),
    softwareHelp: absoluteUrl(isZh ? '/zh' : '/en'),
    featureList: t.features.items.map((item) => item.title),
    offers: [
      {
        '@type': 'Offer',
        price: '0',
        priceCurrency: 'USD',
        name: isZh ? 'MonoSub 免费版' : 'MonoSub Free',
        url: APP_STORE_URL,
        availability: 'https://schema.org/InStock',
      },
      {
        '@type': 'Offer',
        price: '9.99',
        priceCurrency: 'USD',
        name: isZh ? 'MonoSub Pro（App Store 内购，一次性）' : 'MonoSub Pro (App Store in-app purchase, one-time)',
        url: APP_STORE_URL,
        availability: 'https://schema.org/InStock',
      },
      {
        '@type': 'Offer',
        price: '48',
        priceCurrency: 'CNY',
        name: isZh ? 'MonoSub Pro（App Store 内购，一次性）' : 'MonoSub Pro (App Store in-app purchase, one-time)',
        url: APP_STORE_URL,
        availability: 'https://schema.org/InStock',
      },
      {
        '@type': 'Offer',
        price: '8.60',
        priceCurrency: 'USD',
        name: isZh ? 'MonoSub Pro 离线授权（直营店）' : 'MonoSub Pro offline license (direct)',
        url: CHECKOUT_URL,
        availability: 'https://schema.org/InStock',
      },
    ],
    sameAs: [APP_STORE_URL, MS_STORE_URL, REPO_URL],
  };

  const faq: JsonLd = {
    '@type': 'FAQPage',
    '@id': `${pageUrl}#faq`,
    inLanguage: isZh ? 'zh-CN' : 'en',
    mainEntity: t.faq.items.map(({ q, a }) => ({
      '@type': 'Question',
      name: q,
      acceptedAnswer: { '@type': 'Answer', text: a },
    })),
  };

  const howTo: JsonLd = {
    '@type': 'HowTo',
    '@id': `${pageUrl}#howto`,
    name: t.how.title,
    description: t.how.subtitle,
    inLanguage: isZh ? 'zh-CN' : 'en',
    totalTime: 'PT5M',
    step: t.how.steps.map((step, index) => ({
      '@type': 'HowToStep',
      position: index + 1,
      name: step.title,
      text: step.desc,
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
    '@graph': [app, faq, howTo, website],
  };
};
