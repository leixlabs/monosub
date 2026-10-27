import type { Translation } from '../i18n';

/**
 * The FAQ list, in the order the page actually shows it.
 *
 * Shared by `components/FAQ.tsx` (what a visitor reads) and `lib/structuredData.ts`
 * (what the FAQPage markup claims). Google requires the markup to describe the
 * visible Q&A, and a second hand-written copy here would drift the first time a
 * question is reordered — so there is exactly one list.
 *
 * Note the order is deliberate, not source order: OS support and price come
 * first because those are the two questions that decide a purchase.
 */
export const faqPairs = (t: Translation): { q: string; a: string }[] => [
  { q: t.faq_1_q, a: t.faq_1_a },
  { q: t.faq_2_q, a: t.faq_2_a },
  { q: t.faq_5_q, a: t.faq_5_a },
  { q: t.faq_3_q, a: t.faq_3_a },
  { q: t.faq_4_q, a: t.faq_4_a },
  { q: t.faq_6_q, a: t.faq_6_a },
];
