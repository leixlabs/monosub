import type { Translation } from '../i18n';

/**
 * The FAQ list, in the order the page shows it.
 *
 * Now a thin projection of `t.faq.items` — the array lives in the locale files so
 * the FAQPage JSON-LD (structuredData.ts) and the visible `<details>` accordion
 * (components/FAQ.tsx) read exactly the same source. Keeping one list is what
 * stops the markup from describing questions the page no longer shows.
 */
export const faqPairs = (t: Translation): { q: string; a: string }[] => t.faq.items;
