import { useEffect } from 'react';
import { Helmet } from 'react-helmet-async';
import { Link, useParams } from 'react-router-dom';
import { useLanguage } from '../hooks/useLanguage';
import { Navbar } from './Navbar';
import { Footer } from './Footer';
import {
  BLOG_UI,
  CATEGORY_LABEL,
  type Block,
  type Locale,
  type Post,
} from '../content/blog/types';
import { POSTS } from '../content/blog';
import { absoluteUrl } from '../lib/site';
import { blogIndexUrl, blogPostPath, blogPostUrl } from '../lib/seo';
import { APP_STORE_URL, CHECKOUT_URL } from '../lib/links';
import { formatDate, readingMinutes } from '../lib/blogMeta';
import { highPriorityImage } from '../lib/imagePriority';

const renderBlock = (block: Block, index: number) => {
  switch (block.kind) {
    case 'h2':
      return (
        <h2 key={index} className="scroll-mt-24 text-2xl font-semibold text-ink mt-14 mb-5 md:text-3xl">
          {block.text}
        </h2>
      );

    case 'h3':
      return (
        <h3 key={index} className="mt-8 mb-3 text-lg font-semibold text-ink md:text-xl">
          {block.text}
        </h3>
      );

    case 'p':
      return (
        <p key={index} className="mb-5 text-[15px] leading-[1.85] text-ink-7 md:text-base">
          {block.text}
        </p>
      );

    case 'ul':
      return (
        <ul key={index} className="mb-6 space-y-2">
          {block.items.map((item) => (
            <li
              key={item}
              className="relative pl-5 text-[15px] leading-relaxed text-ink-7 before:absolute before:left-0 before:top-[0.65em] before:h-1.5 before:w-1.5 before:rounded-full before:bg-gold/70 before:content-[''] md:text-base"
            >
              {item}
            </li>
          ))}
        </ul>
      );

    case 'callout':
      return (
        <aside key={index} className="card--paper my-8 rounded-lg px-6 py-5">
          {block.title && (
            <p className="mb-2 text-sm font-semibold uppercase tracking-wide text-gold">
              {block.title}
            </p>
          )}
          <p className="m-0 text-[15px] leading-relaxed text-ink-7 md:text-base">{block.text}</p>
        </aside>
      );

    case 'table':
      return (
        <figure key={index} className="my-8">
          {/* .table-wrap handles the responsive stacking (container query); each
              cell carries data-label so the stacked rows keep their headings. */}
          <div className="table-wrap">
            <table>
              <thead>
                <tr>
                  {block.headers.map((header, column) => (
                    <th key={column} scope="col">
                      {header}
                    </th>
                  ))}
                </tr>
              </thead>
              <tbody>
                {block.rows.map((row, rowIndex) => (
                  <tr key={rowIndex}>
                    {row.map((cell, column) => (
                      <td key={column} data-label={block.headers[column]}>
                        {cell}
                      </td>
                    ))}
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
          {block.note && (
            <figcaption className="mt-3 px-1 text-xs leading-relaxed text-ink-5">
              {block.note}
            </figcaption>
          )}
        </figure>
      );
  }
};

/** Compact card for the Related grid at the foot of an article. */
const RelatedCard = ({ post, locale }: { post: Post; locale: Locale }) => {
  const ui = BLOG_UI[locale];
  const content = post[locale];

  return (
    <Link
      to={blogPostPath(locale, post.slug)}
      className="group card card--flat flex flex-col p-5 transition-colors hover:border-gold/40"
    >
      <span className="badge mb-3 self-start">{CATEGORY_LABEL[post.category][locale]}</span>
      <p className="mb-2 font-semibold leading-snug text-ink transition-colors group-hover:text-gold">
        {content.title}
      </p>
      <p className="mt-auto pt-3 text-xs text-ink-5">
        {formatDate(content.updated, locale)} · {ui.readingTime(readingMinutes(post, locale))}
      </p>
    </Link>
  );
};

export const BlogPost = () => {
  const { lang = 'en', slug = '' } = useParams<{ lang: Locale; slug: string }>();
  const { t } = useLanguage();
  const post = POSTS.find((candidate) => candidate.slug === slug);
  const locale: Locale = lang === 'zh' ? 'zh' : 'en';
  const ui = BLOG_UI[locale];

  useEffect(() => {
    document.documentElement.lang = locale;
    document.documentElement.dir = 'ltr';
  }, [locale]);

  // The route is registered only for known slugs, but a bad slug must not render
  // an empty article if someone deep-links it after a rename.
  if (!post) {
    return null;
  }

  const content = post[locale];
  const canonical = blogPostUrl(locale, post.slug);
  const minutes = readingMinutes(post, locale);
  const others = POSTS.filter((candidate) => candidate.slug !== post.slug);

  // Tags double as the article's topic signals, so they are derived rather than
  // invented: the category, the competitor, and the recurring subjects.
  const tags = [CATEGORY_LABEL[post.category][locale], post.competitor, 'MonoSub'].filter(
    (tag, index, list) => list.indexOf(tag) === index
  );

  // BlogPosting markup. `author`/`publisher` point at the product itself: there
  // is no separate editorial entity, and inventing one would be a lie a reader
  // (or a search engine) can catch.
  const articleLd = {
    '@context': 'https://schema.org',
    '@type': 'BlogPosting',
    headline: content.title,
    description: content.description,
    datePublished: content.updated,
    dateModified: content.updated,
    inLanguage: locale === 'zh' ? 'zh-CN' : 'en',
    mainEntityOfPage: canonical,
    articleSection: CATEGORY_LABEL[post.category][locale],
    keywords: tags.join(', '),
    author: { '@type': 'Organization', name: 'MonoSub', url: absoluteUrl('/') },
    publisher: { '@type': 'Organization', name: 'MonoSub', url: absoluteUrl('/') },
    about: { '@type': 'SoftwareApplication', name: 'MonoSub' },
  };

  return (
    <>
      <Helmet>
        <html lang={locale} dir="ltr" />
        <title>{content.metaTitle}</title>
        <meta name="description" content={content.description} />
        <link rel="canonical" href={canonical} />
        <meta property="og:title" content={content.metaTitle} />
        <meta property="og:description" content={content.description} />
        <meta property="og:type" content="article" />
        <meta property="og:url" content={canonical} />
        <meta
          property="og:image"
          content={post.cover ? absoluteUrl(post.cover[locale].src) : absoluteUrl('/og-image.jpg')}
        />
        <meta property="og:image:width" content="1200" />
        <meta property="og:image:height" content="630" />
        <meta property="og:site_name" content="MonoSub" />
        <meta property="og:locale" content={locale === 'zh' ? 'zh_CN' : 'en_US'} />
        <meta property="article:modified_time" content={content.updated} />
        <meta name="twitter:card" content="summary_large_image" />
        <meta name="twitter:title" content={content.metaTitle} />
        <meta name="twitter:description" content={content.description} />
        <script type="application/ld+json">{JSON.stringify(articleLd)}</script>
      </Helmet>

      <div className="min-h-screen bg-paper text-ink-8">
        <Navbar />

        <main className="pt-28 pb-20">
          <div className="container-page">
            <article className="mx-auto max-w-3xl">
              {/* Breadcrumb — the index is the only way back into the blog. */}
              <nav className="mb-8 flex items-center gap-2 text-sm text-ink-5">
                <Link to={`/${locale}`} className="transition-colors hover:text-gold">
                  MonoSub
                </Link>
                <span aria-hidden="true">/</span>
                <Link
                  to={blogIndexUrl(locale).replace(/^https?:\/\/[^/]+/, '')}
                  className="transition-colors hover:text-gold"
                >
                  {ui.indexEyebrow}
                </Link>
              </nav>

              <header>
                {post.cover && (
                  <img
                    src={post.cover[locale].src}
                    alt={post.cover[locale].alt}
                    width={1200}
                    height={630}
                    {...highPriorityImage}
                    className="mb-8 h-auto w-full rounded-lg border border-line"
                  />
                )}
                <p className="eyebrow mb-3">
                  {CATEGORY_LABEL[post.category][locale]} · {post.competitor}
                </p>
                <h1 className="display-2 mb-6 text-ink">{content.title}</h1>
                <div className="flex flex-wrap items-center gap-x-4 gap-y-2 border-b border-line pb-8 text-sm text-ink-5">
                  <time dateTime={content.updated}>
                    {ui.updated} {formatDate(content.updated, locale)}
                  </time>
                  <span aria-hidden="true">·</span>
                  <span>{ui.readingTime(minutes)}</span>
                </div>
              </header>

              <p className="mt-8 mb-2 text-lg leading-[1.85] text-ink-7">{content.intro}</p>

              {content.blocks.map(renderBlock)}

              {/* The verdict is the part an AI answer engine is most likely to
                  quote, so it gets a heading and its own container rather than
                  being the last unstyled paragraph. */}
              <section className="card--paper mt-14 rounded-lg p-7">
                <h2 className="mb-3 text-sm font-semibold uppercase tracking-wide text-gold">
                  {ui.verdict}
                </h2>
                <p className="m-0 leading-[1.8] text-ink-8">{content.verdict}</p>
              </section>

              {/* Tags, then the author box: the two things a reader checks before
                  deciding whether to trust and share an article. */}
              <div className="mt-10 flex flex-wrap gap-2">
                {tags.map((tag) => (
                  <span
                    key={tag}
                    className="rounded-full border border-line px-3 py-1 text-xs text-ink-6"
                  >
                    #{tag}
                  </span>
                ))}
              </div>

              <div className="card card--flat mt-8 flex items-start gap-4 p-6">
                <div className="flex h-11 w-11 shrink-0 items-center justify-center rounded-full bg-ink font-display font-semibold text-paper">
                  M
                </div>
                <div>
                  <p className="mb-1 font-semibold text-ink">{ui.authorName}</p>
                  <p className="m-0 text-sm leading-relaxed text-ink-6">{ui.authorBio}</p>
                </div>
              </div>

              <p className="mt-6 text-xs leading-relaxed text-ink-5">{ui.disclosure}</p>

              <section className="card--paper mt-12 rounded-lg p-7">
                <h2 className="mb-2 text-xl font-semibold text-ink">{ui.ctaTitle}</h2>
                <p className="mb-5 text-sm leading-relaxed text-ink-6">{ui.ctaBody}</p>
                <div className="flex flex-wrap gap-3">
                  <a
                    href={APP_STORE_URL}
                    target="_blank"
                    rel="noopener noreferrer"
                    className="btn btn-primary"
                  >
                    {t.nav.cta_primary}
                  </a>
                  <a
                    href={CHECKOUT_URL}
                    target="_blank"
                    rel="noopener noreferrer"
                    className="btn btn-secondary"
                  >
                    {t.nav.cta_secondary}
                  </a>
                </div>
              </section>

              {others.length > 0 && (
                <section className="mt-14">
                  <h2 className="mb-5 text-xl font-semibold text-ink">{ui.related}</h2>
                  <div className="grid gap-4 sm:grid-cols-2">
                    {others.map((other) => (
                      <RelatedCard key={other.slug} post={other} locale={locale} />
                    ))}
                  </div>
                </section>
              )}

              <nav className="mt-10">
                <Link
                  to={blogIndexUrl(locale).replace(/^https?:\/\/[^/]+/, '')}
                  className="text-sm text-ink-6 transition-colors hover:text-gold"
                >
                  ← {ui.backToIndex}
                </Link>
              </nav>
            </article>
          </div>
        </main>

        <Footer />
      </div>
    </>
  );
};

export default BlogPost;
