import { useEffect, useMemo, useState } from 'react';
import { Helmet } from 'react-helmet-async';
import { Link, useParams } from 'react-router-dom';
import { useLanguage } from '../hooks/useLanguage';
import { Navbar } from './Navbar';
import { Footer } from './Footer';
import { POSTS } from '../content/blog';
import {
  BLOG_UI,
  CATEGORY_LABEL,
  CATEGORY_ORDER,
  type Category,
  type Locale,
  type Post,
} from '../content/blog/types';
import { blogIndexUrl, blogPostPath } from '../lib/seo';
import { absoluteUrl } from '../lib/site';
import { formatDate, readingMinutes } from '../lib/blogMeta';
import { highPriorityImage, lowPriorityImage } from '../lib/imagePriority';

/**
 * The index follows the shape a reader actually arrives with: a statement of what
 * this blog is, a row of category chips, then one featured piece and a grid.
 *
 * The featured/more split is not decoration — the first card gets a larger type
 * hierarchy and the whole teaser, which is what makes a blog look alive when
 * there are only a handful of posts. It disappears when a filter is active,
 * because "featured" only means "newest overall", not "newest here".
 */

const CategoryChip = ({
  active,
  label,
  onClick,
}: {
  active: boolean;
  label: string;
  onClick: () => void;
}) => (
  <button
    type="button"
    onClick={onClick}
    aria-pressed={active}
    className={`px-4 py-1.5 rounded-full text-sm whitespace-nowrap transition-colors ${
      active
        ? 'bg-white text-black font-medium'
        : 'bg-white/[0.06] text-gray-300 hover:bg-white/[0.12] hover:text-white'
    }`}
  >
    {label}
  </button>
);

/**
 * The big card. Image-free by design: there are no photographs to show, and a
 * stock image would read as filler. Typography carries it instead.
 */
const FeaturedCard = ({ post, locale }: { post: Post; locale: Locale }) => {
  const ui = BLOG_UI[locale];
  const content = post[locale];
  const cover = post.cover?.[locale];

  return (
    <Link
      to={blogPostPath(locale, post.slug)}
      className="group relative block overflow-hidden rounded-3xl border border-primary/25 bg-dark-card transition-colors hover:border-primary/60"
    >
      {/* A soft wash from the accent, so the featured card reads as a different
          class of object than the grid below rather than just a wider box. */}
      <div
        aria-hidden="true"
        className="pointer-events-none absolute inset-0 bg-gradient-to-br from-primary/[0.14] via-transparent to-accent/[0.08]"
      />

      <div className="relative">
        {cover && (
          <div className="overflow-hidden border-b border-dark-border bg-black/40">
            <img
              src={cover.src}
              alt={cover.alt}
              width={1200}
              height={630}
              /* The featured card is the largest element above the fold on this
                 page, so its image is the LCP candidate — load it eagerly and let
                 the browser prioritise it. Everything below lazy-loads. */
              {...highPriorityImage}
              className="w-full h-auto transition-transform duration-500 group-hover:scale-[1.02]"
            />
          </div>
        )}

        <div className="p-8 md:p-12">
          <div className="flex items-center gap-3 mb-5">
            <span className="px-2.5 py-1 rounded-full bg-primary/20 text-xs text-primary font-medium">
              {CATEGORY_LABEL[post.category][locale]}
            </span>
            <span className="text-xs text-gray-500 uppercase tracking-wider">{ui.featured}</span>
          </div>

          <h2 className="text-2xl md:text-[2.5rem] font-bold leading-[1.12] tracking-tight mb-4 group-hover:text-primary transition-colors">
            {content.title}
          </h2>

          <p className="text-gray-300 leading-relaxed max-w-2xl mb-7 text-base md:text-lg">
            {post.teaser?.[locale] ?? content.description}
          </p>

          <div className="flex flex-wrap items-center gap-x-3 gap-y-1 text-sm text-gray-500">
            <span>{post.competitor}</span>
            <span aria-hidden="true">·</span>
            <time dateTime={content.updated}>{formatDate(content.updated, locale)}</time>
            <span aria-hidden="true">·</span>
            <span>{ui.readingTime(readingMinutes(post, locale))}</span>
          </div>
        </div>
      </div>
    </Link>
  );
};

const PostCard = ({ post, locale }: { post: Post; locale: Locale }) => {
  const ui = BLOG_UI[locale];
  const content = post[locale];
  const cover = post.cover?.[locale];

  return (
    <Link
      to={blogPostPath(locale, post.slug)}
      className="group flex flex-col overflow-hidden rounded-2xl border border-dark-border bg-white/[0.02] transition-colors hover:border-primary/50 hover:bg-white/[0.04]"
    >
      {cover && (
        <div className="overflow-hidden border-b border-dark-border bg-black/40">
          <img
            src={cover.src}
            alt={cover.alt}
            width={1200}
            height={630}
            {...lowPriorityImage}
            className="w-full h-auto transition-transform duration-500 group-hover:scale-[1.03]"
          />
        </div>
      )}

      <div className="flex flex-col flex-1 p-6">
        <span className="self-start px-2.5 py-1 rounded-full bg-white/10 text-xs text-gray-300 mb-4">
          {CATEGORY_LABEL[post.category][locale]}
        </span>

        <h3 className="text-lg md:text-xl font-bold leading-snug mb-3 group-hover:text-primary transition-colors">
          {content.title}
        </h3>

        <p className="text-sm text-gray-400 leading-relaxed mb-6 line-clamp-3">
          {post.teaser?.[locale] ?? content.description}
        </p>

        <div className="mt-auto flex items-center gap-2 text-xs text-gray-500">
          <time dateTime={content.updated}>{formatDate(content.updated, locale)}</time>
          <span aria-hidden="true">·</span>
          <span>{ui.readingTime(readingMinutes(post, locale))}</span>
        </div>
      </div>
    </Link>
  );
};

export const BlogIndex = () => {
  const { lang = 'en' } = useParams<{ lang: Locale }>();
  const locale: Locale = lang === 'zh' ? 'zh' : 'en';
  const ui = BLOG_UI[locale];
  const { t } = useLanguage();

  /**
   * The filter is component state, not a route: it is a view of one list, and a
   * URL per chip would create near-duplicate pages for a handful of posts. The
   * category links at the bottom still exist as real URLs, so nothing is
   * unreachable to a crawler.
   */
  const [active, setActive] = useState<Category | 'all'>('all');

  useEffect(() => {
    document.documentElement.lang = locale;
    document.documentElement.dir = 'ltr';
  }, [locale]);

  /**
   * Newest first, with a tie broken by the authored order in `POSTS`.
   *
   * The comparator must return 0 for equal dates: returning -1 (as a naive
   * `b > a ? 1 : -1` does) makes the result depend on the engine's sort strategy
   * and would let the featured post differ between the SSR pass and the browser.
   */
  const sorted = useMemo(
    () =>
      [...POSTS].sort((a, b) => {
        if (a[locale].updated === b[locale].updated) {
          return POSTS.indexOf(a) - POSTS.indexOf(b);
        }
        return b[locale].updated.localeCompare(a[locale].updated);
      }),
    [locale]
  );

  const visible = active === 'all' ? sorted : sorted.filter((post) => post.category === active);

  // Only offer chips for categories that actually have posts — an empty tab is a
  // dead end for the reader.
  const categories = CATEGORY_ORDER.filter((category) =>
    POSTS.some((post) => post.category === category)
  );

  const featured = active === 'all' ? visible[0] : undefined;
  const rest = featured ? visible.slice(1) : visible;

  const canonical = blogIndexUrl(locale);

  const listLd = {
    '@context': 'https://schema.org',
    '@type': 'Blog',
    name: ui.indexTitle,
    description: ui.indexSubtitle,
    inLanguage: locale === 'zh' ? 'zh-CN' : 'en',
    url: canonical,
    blogPost: sorted.map((post) => ({
      '@type': 'BlogPosting',
      headline: post[locale].title,
      url: absoluteUrl(blogPostPath(locale, post.slug)),
      datePublished: post[locale].updated,
      dateModified: post[locale].updated,
    })),
  };

  return (
    <>
      <Helmet>
        <html lang={locale} dir="ltr" />
        <title>{`${ui.indexEyebrow} · MonoSub`}</title>
        <meta name="description" content={ui.indexSubtitle} />
        <link rel="canonical" href={canonical} />
        <link rel="alternate" hrefLang="zh-CN" href={blogIndexUrl('zh')} />
        <link rel="alternate" hrefLang="zh" href={blogIndexUrl('zh')} />
        <link rel="alternate" hrefLang="en" href={blogIndexUrl('en')} />
        <link rel="alternate" hrefLang="x-default" href={blogIndexUrl('en')} />
        <meta property="og:title" content={`${ui.indexEyebrow} · MonoSub`} />
        <meta property="og:description" content={ui.indexSubtitle} />
        <meta property="og:type" content="website" />
        <meta property="og:url" content={canonical} />
        <meta property="og:image" content={absoluteUrl('/og-image.jpg')} />
        <meta property="og:site_name" content="MonoSub" />
        <meta name="twitter:card" content="summary_large_image" />
        <script type="application/ld+json">{JSON.stringify(listLd)}</script>
      </Helmet>

      <div className="min-h-screen bg-dark-bg text-white">
        <Navbar />

        <main className="px-6 pt-32 pb-24">
          <div className="max-w-4xl mx-auto">
            {/* Eyebrow + oversized heading: the strongest signal that this is
                editorial rather than another product page. */}
            <p className="text-primary text-sm font-semibold tracking-wide mb-4">
              {ui.indexEyebrow}
            </p>
            <h1 className="text-[2.1rem] md:text-5xl font-bold leading-[1.12] tracking-tight mb-5">
              {ui.indexTitle}
            </h1>
            <p className="text-lg text-gray-400 leading-relaxed mb-9 max-w-2xl">{ui.indexSubtitle}</p>

            <div className="flex flex-wrap gap-2 mb-10" role="group" aria-label={ui.indexEyebrow}>
              <CategoryChip
                active={active === 'all'}
                label={ui.allPosts}
                onClick={() => setActive('all')}
              />
              {categories.map((category) => (
                <CategoryChip
                  key={category}
                  active={active === category}
                  label={CATEGORY_LABEL[category][locale]}
                  onClick={() => setActive(category)}
                />
              ))}
            </div>

            {visible.length === 0 && (
              <p className="text-gray-500 py-16 text-center">{ui.emptyCategory}</p>
            )}

            {featured && (
              <div className="mb-8">
                <FeaturedCard post={featured} locale={locale} />
              </div>
            )}

            {rest.length > 0 && (
              <>
                {featured && (
                  <h2 className="text-sm font-semibold text-gray-500 uppercase tracking-wider mt-12 mb-4">
                    {ui.morePosts}
                  </h2>
                )}
                <div className="grid gap-4 sm:grid-cols-2">
                  {rest.map((post) => (
                    <PostCard key={post.slug} post={post} locale={locale} />
                  ))}
                </div>
              </>
            )}

            <p className="text-xs text-gray-600 mt-12 leading-relaxed">{ui.disclosure}</p>

            <section className="mt-14 rounded-3xl border border-primary/30 bg-gradient-to-b from-primary/10 to-transparent p-8 md:p-10">
              <h2 className="text-2xl font-bold mb-2">{ui.ctaTitle}</h2>
              <p className="text-gray-400 text-sm mb-6 leading-relaxed max-w-xl">{ui.ctaBody}</p>
              <Link
                to={`/${locale}`}
                className="cta-button inline-block text-white text-sm font-semibold px-6 py-3 rounded-full"
              >
                {t.nav_cta}
              </Link>
            </section>
          </div>
        </main>

        <Footer />
      </div>
    </>
  );
};

export default BlogIndex;
