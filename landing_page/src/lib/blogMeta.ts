import type { Locale, Post } from '../content/blog/types';

/**
 * Reading time for a post, in whole minutes.
 *
 * Two different rates because CJK and Latin text are not comparable by word
 * count: ~300 Chinese characters per minute against ~200 English words. Counting
 * both and adding keeps the estimate honest for the mixed copy these posts
 * actually contain (Chinese quotes inside English paragraphs, and vice versa).
 */
export const readingMinutes = (post: Post, locale: Locale): number => {
  const content = post[locale];
  const text = [
    content.title,
    content.intro,
    content.verdict,
    ...content.blocks.flatMap((block) => {
      if (block.kind === 'ul') return block.items;
      if (block.kind === 'table') return block.rows.flat();
      if (block.kind === 'callout') return [block.text];
      return [block.text];
    }),
  ].join(' ');

  const cjk = (text.match(/[\u4e00-\u9fff]/g) ?? []).length;
  const latin = text.replace(/[\u4e00-\u9fff]/g, ' ').split(/\s+/).filter(Boolean).length;
  return Math.max(1, Math.round(cjk / 300 + latin / 200));
};

const MONTHS_EN = [
  'Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun',
  'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec',
];

/**
 * `2026-10-27` → `2026年10月27日` / `27 Oct 2026`.
 *
 * Hand-rolled rather than `toLocaleDateString`: the prerender and the browser
 * must produce identical markup, and ICU output differs between Node builds and
 * browsers (and shifts with the system timezone once a Date object is involved).
 * Parsing the ISO string as parts sidesteps both.
 */
export const formatDate = (iso: string, locale: Locale): string => {
  const [year, month, day] = iso.split('-').map(Number);
  if (!year || !month || !day) return iso;
  if (locale === 'zh') return `${year}年${month}月${day}日`;
  return `${day} ${MONTHS_EN[month - 1]} ${year}`;
};
