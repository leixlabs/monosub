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
import { STORE_URL, RELEASES_URL } from '../lib/links';
import { formatDate, readingMinutes } from '../lib/blogMeta';
import { highPriorityImage } from '../lib/imagePriority';

const renderBlock = (block: Block, index: number) => {
  switch (block.kind) {
    case 'h2':
      return (
        <h2 key={index} className="text-2xl md:text-3xl font-bold mt-14 mb-5 scroll-mt-24 text-white">
          {block.text}
        </h2>
      );

    case 'h3':
      return (
        <h3 key={index} className="text-lg md:text-xl font-semibold mt-8 mb-3 text-white">
          {block.text}
        </h3>
      );

    case 'p':
      return (
        <p key={index} className="text-gray-300 leading-[1.85] mb-5 text-[15px] md:text-base">
          {block.text}
        </p>
      );

    case 'ul':
      return (
        <ul key={index} className="mb-6 space-y-2">
          {block.items.map((item) => (
            <li
              key={item}
              className="text-gray-300 leading-relaxed text-[15px] md:text-base pl-5 relative before:content-[''] before:absolute before:left-0 before:top-[0.65em] before:w-1.5 before:h-1.5 before:rounded-full before:bg-primary/70"
            >
              {item}
            </li>
          ))}
        </ul>
      );

    case 'callout':
      return (
        <aside key={index} className="my-8 rounded-2xl border border-primary/30 bg-primary/5 px-6 py-5">
          {block.title && (
            <p className="font-semibold text-primary mb-2 text-sm uppercase tracking-wide">
              {block.title}
            </p>
          )}
          <p className="text-gray-300 leading-relaxed text-[15px] md:text-base m-0">{block.text}</p>
        </aside>
      );

    case 'table':
      return (
        <figure key={index} className="my-8">
          {/* Horizontal scroll rather than a cramped table: comparison rows are
              wide, and squeezing them breaks the one thing they exist for. */}
          <div className="overflow-x-auto rounded-2xl border border-dark-border">
            <table className="w-full text-left text-sm border-collapse min-w-[560px]">
              <thead>
                <tr className="bg-dark-card">
                  {block.headers.map((header, column) => (
                    <th
                      key={column}
                      scope="col"
                      className={`px-4 py-3 font-semibold text-white/90 border-b border-dark-border whitespace-nowrap ${
                        column === 0 ? 'text-gray-400 font-medium' : ''
                      }`}
                    >
                      {header}
                    </th>
                  ))}
                </tr>
              </thead>
              <tbody>
                {block.rows.map((row, rowIndex) => (
                  <tr key={rowIndex} className="border-b border-dark-border/60 last:border-0">
                    {row.map((cell, column) => (
                      <td
                        key={column}
                        className={`px-4 py-3 align-top leading-relaxed ${
                          column === 0 ? 'text-gray-400' : 'text-gray-300'
                        }`}
                      >
                        {cell}
                      </td>
                    ))}
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
          {block.note && (
            <figcaption className="text-xs text-gray-500 leading-relaxed mt-3 px-1">
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
      className="group flex flex-col rounded-2xl border border-dark-border bg-white/[0.02] p-5 transition-colors hover:border-primary/50 hover:bg-white/[0.04]"
    >
      <span className="self-start px-2 py-0.5 rounded-full bg-white/10 text-[11px] text-gray-300 mb-3">
        {CATEGORY_LABEL[post.category][locale]}
      </span>
      <p className="text-white font-semibold leading-snug mb-2 group-hover:text-primary transition-colors">
        {content.title}
      </p>
      <p className="text-xs text-gray-500 mt-auto pt-3">
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

      <div className="min-h-screen bg-dark-bg text-white">
        <Navbar />

        <main className="px-6 pt-28 pb-20">
          <article className="max-w-3xl mx-auto">
            {/* Breadcrumb — the index is the only way back into the blog. */}
            <nav className="mb-8 flex items-center gap-2 text-sm text-gray-500">
              <Link to={`/${locale}`} className="hover:text-primary transition-colors">
                MonoSub
              </Link>
              <span aria-hidden="true">/</span>
              <Link to={blogIndexUrl(locale).replace(/^https?:\/\/[^/]+/, '')} className="hover:text-primary transition-colors">
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
                  className="w-full h-auto rounded-2xl border border-dark-border mb-8"
                />
              )}
              <p className="text-primary text-sm font-semibold tracking-wide mb-3">
                {CATEGORY_LABEL[post.category][locale]} · {post.competitor}
              </p>
              <h1 className="text-3xl md:text-[2.6rem] font-bold leading-tight mb-6">
                {content.title}
              </h1>
              <div className="flex flex-wrap items-center gap-x-4 gap-y-2 text-sm text-gray-500 pb-8 border-b border-dark-border">
                <time dateTime={content.updated}>
                  {ui.updated} {formatDate(content.updated, locale)}
                </time>
                <span aria-hidden="true">·</span>
                <span>{ui.readingTime(minutes)}</span>
              </div>
            </header>

            <p className="text-lg text-gray-200 leading-[1.85] mt-8 mb-2">{content.intro}</p>

            {content.blocks.map(renderBlock)}

            {/* The verdict is the part an AI answer engine is most likely to
                quote, so it gets a heading and its own container rather than
                being the last unstyled paragraph. */}
            <section className="mt-14 rounded-2xl border border-dark-border bg-dark-card p-7">
              <h2 className="text-sm font-semibold text-primary uppercase tracking-wide mb-3">
                {ui.verdict}
              </h2>
              <p className="text-gray-200 leading-[1.8] m-0">{content.verdict}</p>
            </section>

            {/* Tags, then the author box: the two things a reader checks before
                deciding whether to trust and share an article. */}
            <div className="mt-10 flex flex-wrap gap-2">
              {tags.map((tag) => (
                <span
                  key={tag}
                  className="px-3 py-1 rounded-full border border-dark-border text-xs text-gray-400"
                >
                  #{tag}
                </span>
              ))}
            </div>

            <div className="mt-8 flex items-start gap-4 rounded-2xl border border-dark-border bg-white/[0.02] p-6">
              <div className="w-11 h-11 rounded-full bg-gradient-to-br from-primary to-accent flex items-center justify-center font-bold text-white shrink-0">
                M
              </div>
              <div>
                <p className="font-semibold text-white mb-1">{ui.authorName}</p>
                <p className="text-sm text-gray-400 leading-relaxed m-0">{ui.authorBio}</p>
              </div>
            </div>

            <p className="text-xs text-gray-600 mt-6 leading-relaxed">{ui.disclosure}</p>

            <section className="mt-12 rounded-2xl border border-primary/30 bg-gradient-to-b from-primary/10 to-transparent p-7">
              <h2 className="text-xl font-bold mb-2">{ui.ctaTitle}</h2>
              <p className="text-gray-400 text-sm mb-5 leading-relaxed">{ui.ctaBody}</p>
              <div className="flex flex-wrap gap-3">
                <a
                  href={STORE_URL}
                  target="_blank"
                  rel="noopener noreferrer"
                  className="cta-button text-white text-sm font-semibold px-5 py-2.5 rounded-full"
                >
                  {t.nav_cta}
                </a>
                <a
                  href={RELEASES_URL}
                  target="_blank"
                  rel="noopener noreferrer"
                  className="text-sm font-semibold px-5 py-2.5 rounded-full border border-dark-border hover:border-primary transition-colors"
                >
                  {t.hero_cta_mac}
                </a>
              </div>
            </section>

            {others.length > 0 && (
              <section className="mt-14">
                <h2 className="text-xl font-bold mb-5">{ui.related}</h2>
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
                className="text-sm text-gray-400 hover:text-primary transition-colors"
              >
                ← {ui.backToIndex}
              </Link>
            </nav>
          </article>
        </main>

        <Footer />
      </div>
    </>
  );
};

export default BlogPost;
