/**
 * The content model for the blog.
 *
 * Posts are data, not JSX: every article is one typed object per locale, and a
 * single renderer turns it into semantic HTML. That buys three things the blog
 * needs and a pile of hand-written JSX would not:
 *
 * - **Prerendering.** `scripts/prerender.mjs` renders each post through the same
 *   React tree as the rest of the site, so the article text is in the served
 *   HTML. Article bodies are exactly what a crawler that never runs JS would
 *   otherwise miss.
 * - **No drift between locales.** `Post` requires both `zh` and `en`, so a post
 *   cannot ship in one language by accident.
 * - **Structured data.** Sections carry their own shape (`table`, `h2`, …), which
 *   is what lets the JSON-LD and the sitemap be derived instead of retyped.
 */

/** A block of article body. Deliberately small: this is prose, not a CMS. */
export type Block =
  | { kind: 'p'; text: string }
  | { kind: 'h2'; text: string }
  | { kind: 'h3'; text: string }
  | { kind: 'ul'; items: string[] }
  /** `note` renders under the table as a caveat (usually a "prices as of …"). */
  | { kind: 'table'; headers: string[]; rows: string[][]; note?: string }
  /** Highlighted aside — used for the "where the other tool wins" honesty box. */
  | { kind: 'callout'; title?: string; text: string };

export type Locale = 'zh' | 'en';

/**
 * Post categories. The set mirrors how a reader arrives: looking to buy
 * (`comparison`), looking to do a thing (`tutorial`, `tips`), or checking what
 * changed (`product`).
 *
 * Only categories that at least one post uses are shown as filters, so this can
 * stay ahead of the content without shipping empty tabs.
 */
export type Category = 'comparison' | 'tutorial' | 'tips' | 'product';

export const CATEGORY_LABEL: Record<Category, Record<Locale, string>> = {
  comparison: { zh: '对比', en: 'Comparisons' },
  tutorial: { zh: '教程', en: 'Tutorials' },
  tips: { zh: '技巧', en: 'Tips' },
  product: { zh: '产品更新', en: 'Product' },
};

/** Display order of the filter chips, so the row does not reshuffle per locale. */
export const CATEGORY_ORDER: Category[] = ['comparison', 'tutorial', 'tips', 'product'];

export interface PostContent {
  /** `<h1>`. Kept separate from `metaTitle` because search results want the
   *  keyword up front while the page can read more like a sentence. */
  title: string;
  metaTitle: string;
  description: string;
  /** ISO date. Rendered per locale, and fed to `BlogPosting.dateModified`. */
  updated: string;
  /** The lede — one short paragraph above the first heading. */
  intro: string;
  blocks: Block[];
  /** Closing paragraph, rendered as the emphasised verdict box. */
  verdict: string;
}

export interface Post {
  /** URL segment: `<lang>/blog/<slug>/`. Stable — it is the canonical URL. */
  slug: string;
  category: Category;
  /** Name of the tool being compared, shown on the index card. */
  competitor: string;
  /** Short label for the card, e.g. "免费开源 · 跨平台". */
  tagline: Record<Locale, string>;
  /**
   * One line for the index card, distinct from `description` (which is written
   * for search results and gets truncated there). Optional: a post with no
   * teaser falls back to `description`.
   */
  teaser?: Record<Locale, string>;
  /**
   * Optional card image, `1200×630` (the same ratio as `og-image.jpg`).
   *
   * Lives under `public/blog/`. Per-locale because a screenshot of the app in
   * Chinese should not sit on the English card — the same reason the product
   * screenshots ship twice.
   *
   * Optional on purpose: the layout must look deliberate without one, so a post
   * can ship before its image does. When absent, the card leans on typography.
   */
  cover?: Record<Locale, { src: string; alt: string }>;
  zh: PostContent;
  en: PostContent;
}

/** Per-locale UI strings the blog renderer needs but content does not own. */
export const BLOG_UI: Record<
  Locale,
  {
    updated: string;
    verdict: string;
    related: string;
    backToIndex: string;
    ctaTitle: string;
    ctaBody: string;
    ctaButton: string;
    readingTime: (minutes: number) => string;
    indexEyebrow: string;
    indexTitle: string;
    indexSubtitle: string;
    allPosts: string;
    featured: string;
    morePosts: string;
    emptyCategory: string;
    disclosure: string;
    authorName: string;
    authorBio: string;
    tags: string;
  }
> = {
  zh: {
    updated: '更新于',
    verdict: '结论',
    related: '相关内容',
    backToIndex: '全部文章',
    ctaTitle: '先试试，再决定要不要买',
    ctaBody: 'Windows 版在微软商店，macOS 版从 GitHub Releases 免费获取。两个版本功能相同。',
    ctaButton: '查看获取方式',
    readingTime: (minutes) => `${minutes} 分钟阅读`,
    indexEyebrow: '博客',
    indexTitle: '关于字幕、转写和本地 AI 的笔记。',
    indexSubtitle: '对比、教程和踩坑记录。写清楚每个工具适合谁，包括 MonoSub 不如人的地方。',
    allPosts: '全部',
    featured: '最新',
    morePosts: '更多文章',
    emptyCategory: '这个分类下还没有文章。',
    disclosure: '价格与功能以各产品官网为准，本文数据核对于文中标注日期。',
    authorName: 'MonoSub',
    authorBio: '做本地字幕工具的人写的对比与教程。产品能做什么、做不到什么，都写在里面。',
    tags: '标签',
  },
  en: {
    updated: 'Updated',
    verdict: 'The verdict',
    related: 'Related reading',
    backToIndex: 'All posts',
    ctaTitle: 'Try it before you pay',
    ctaBody:
      'Windows ships through the Microsoft Store; the macOS build is free from GitHub Releases. Both are the same program.',
    ctaButton: 'See how to get it',
    readingTime: (minutes) => `${minutes} min read`,
    indexEyebrow: 'Blog',
    indexTitle: 'Notes on subtitles, transcription and local AI.',
    indexSubtitle:
      'Comparisons, tutorials and the things that broke. Each one says who a tool suits — including where MonoSub loses.',
    allPosts: 'All',
    featured: 'Latest',
    morePosts: 'More posts',
    emptyCategory: 'Nothing in this category yet.',
    disclosure: 'Prices and features follow each vendor’s site; data was checked on the date noted in each post.',
    authorName: 'MonoSub',
    authorBio: 'Written by the people building a local subtitle tool. What it does and does not do is all in here.',
    tags: 'Tags',
  },
};
