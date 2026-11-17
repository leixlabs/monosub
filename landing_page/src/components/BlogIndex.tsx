import { useEffect, useMemo, useState } from 'react';
import { Helmet } from 'react-helmet-async';
import { Link, useParams } from 'react-router-dom';
import { useLanguage } from '../hooks/useLanguage';
import { usePlatform } from '../hooks/usePlatform';
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
import { downloadUrlFor } from '../lib/links';

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
    className={`rounded-full border px-4 py-1.5 text-sm whitespace-nowrap transition-colors ${
      active
        ? 'border-gold/40 bg-gold-200 font-medium text-gold-800'
        : 'border-line text-ink-6 hover:border-ink-5 hover:text-ink'
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
      className="group card relative block overflow-hidden transition-colors hover:border-gold/40"
    >
      <div className="relative">
        {cover && (
          <div className="overflow-hidden border-b border-line bg-paper-2">
            <img
              src={cover.src}
              alt={cover.alt}
              width={1200}
              height={630}
              /* The featured card is the largest element above the fold on this
                 page, so its image is the LCP candidate — load it eagerly and let
                 the browser prioritise it. Everything below lazy-loads. */
              {...highPriorityImage}
              className="h-auto w-full transition-transform duration-500 group-hover:scale-[1.02]"
            />
          </div>
        )}

        <div className="p-8 md:p-12">
          <div className="mb-5 flex items-center gap-3">
            <span className="badge badge--gold">{CATEGORY_LABEL[post.category][locale]}</span>
            <span className="index-num">{ui.featured}</span>
          </div>

          <h2 className="mb-4 text-2xl font-semibold leading-[1.14] tracking-tight text-ink transition-colors group-hover:text-gold md:text-[2.4rem]">
            {content.title}
          </h2>

          <p className="mb-7 max-w-2xl text-base leading-relaxed text-ink-6 md:text-lg">
            {post.teaser?.[locale] ?? content.description}
          </p>

          <div className="flex flex-wrap items-center gap-x-3 gap-y-1 text-sm text-ink-5">
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
      className="group card card--flat flex flex-col overflow-hidden transition-colors hover:border-gold/40"
    >
      {cover && (
        <div className="overflow-hidden border-b border-line bg-paper-2">
          <img
            src={cover.src}
            alt={cover.alt}
            width={1200}
            height={630}
            {...lowPriorityImage}
            className="h-auto w-full transition-transform duration-500 group-hover:scale-[1.03]"
          />
        </div>
      )}

      <div className="flex flex-1 flex-col p-6">
        <span className="badge mb-4 self-start">{CATEGORY_LABEL[post.category][locale]}</span>

        <h3 className="mb-3 text-lg font-semibold leading-snug text-ink transition-colors group-hover:text-gold md:text-xl">
          {content.title}
        </h3>

        <p className="mb-6 line-clamp-3 text-sm leading-relaxed text-ink-6">
          {post.teaser?.[locale] ?? content.description}
        </p>

        <div className="mt-auto flex items-center gap-2 text-xs text-ink-5">
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
  const platform = usePlatform();
  const ctaLabel =
    platform === 'windows' ? t.cta.windows : platform === 'mac' ? t.cta.mac : t.cta.generic;

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

      <div className="min-h-screen bg-paper text-ink-8">
        <Navbar />

        <main className="pt-32 pb-24">
          <div className="container-page">
            <div className="mx-auto max-w-4xl">
              {/* Eyebrow + oversized heading: the strongest signal that this is
                  editorial rather than another product page. */}
              <p className="eyebrow mb-4">{ui.indexEyebrow}</p>
              <h1 className="display-2 mb-5 text-ink">{ui.indexTitle}</h1>
              <p className="mb-9 max-w-2xl text-lg leading-relaxed text-ink-6">{ui.indexSubtitle}</p>

              <div className="mb-10 flex flex-wrap gap-2" role="group" aria-label={ui.indexEyebrow}>
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
                <p className="muted py-16 text-center">{ui.emptyCategory}</p>
              )}

              {featured && (
                <div className="mb-8">
                  <FeaturedCard post={featured} locale={locale} />
                </div>
              )}

              {rest.length > 0 && (
                <>
                  {featured && (
                    <h2 className="index-num mt-12 mb-4 block">{ui.morePosts}</h2>
                  )}
                  <div className="grid gap-4 sm:grid-cols-2">
                    {rest.map((post) => (
                      <PostCard key={post.slug} post={post} locale={locale} />
                    ))}
                  </div>
                </>
              )}

              <p className="mt-12 text-xs leading-relaxed text-ink-5">{ui.disclosure}</p>

              <section className="ink-band mt-14 rounded-lg p-8 md:p-10">
                <h2 className="mb-2 text-2xl">{ui.ctaTitle}</h2>
                <p className="muted mb-6 max-w-xl text-sm leading-relaxed">{ui.ctaBody}</p>
                <a
                  href={downloadUrlFor(platform)}
                  target="_blank"
                  rel="noopener noreferrer"
                  className="btn btn-primary"
                >
                  {ctaLabel}
                </a>
              </section>
            </div>
          </div>
        </main>

        <Footer />
      </div>
    </>
  );
};

export default BlogIndex;
