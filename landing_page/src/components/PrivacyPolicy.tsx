import { useEffect } from 'react';
import { Helmet } from 'react-helmet-async';
import { Link } from 'react-router-dom';
import { useLanguage } from '../hooks/useLanguage';
import { LanguageSwitcher } from './LanguageSwitcher';
import { Footer } from './Footer';
import { privacyContent } from '../i18n/privacy';
import { alternateLinks, pageUrl } from '../lib/seo';

export const PrivacyPolicy = () => {
  const { lang } = useLanguage();
  const content = privacyContent[lang] || privacyContent.en;
  const canonical = pageUrl(lang, 'privacy');

  useEffect(() => {
    document.documentElement.lang = lang;
    document.documentElement.dir = 'ltr';
  }, [lang]);

  return (
    <>
      <Helmet>
        <html lang={lang} dir="ltr" />
        <title>{content.meta_title}</title>
        <meta name="description" content={content.meta_description} />

        {/* Canonical */}
        <link rel="canonical" href={canonical} />

        {/* Hreflang tags */}
        {alternateLinks('privacy').map((link) => (
          <link key={link.hrefLang} rel="alternate" hrefLang={link.hrefLang} href={link.href} />
        ))}

        {/* Open Graph */}
        <meta property="og:title" content={content.meta_title} />
        <meta property="og:description" content={content.meta_description} />
        <meta property="og:type" content="website" />
        <meta property="og:url" content={canonical} />

        {/* Twitter Card */}
        <meta name="twitter:card" content="summary" />
        <meta name="twitter:title" content={content.meta_title} />
        <meta name="twitter:description" content={content.meta_description} />

        {/* A policy page should not compete with the product page for the brand. */}
        <meta name="robots" content="noindex, follow" />
      </Helmet>

      <div className="min-h-screen bg-paper text-ink-8">
        <nav className="fixed left-0 right-0 top-0 z-50 border-b border-line bg-paper/85 px-6 py-4 backdrop-blur-lg">
          <div className="mx-auto flex max-w-7xl items-center justify-between">
            <Link to={`/${lang}`} className="font-display text-2xl font-semibold text-ink">
              MonoSub
            </Link>
            <div className="flex items-center gap-4">
              <Link
                to={`/${lang}`}
                className="hidden text-sm text-ink-6 transition-colors hover:text-ink sm:inline"
              >
                {content.back_home}
              </Link>
              <LanguageSwitcher />
            </div>
          </div>
        </nav>

        <main className="pt-28 pb-16">
          <div className="container-page">
            <div className="mx-auto max-w-3xl">
              <p className="text-sm text-ink-5">
                {content.last_updated_label}: {content.last_updated}
              </p>
              <h1 className="display-2 mt-2 mb-6 text-ink">{content.title}</h1>
              <p className="mb-12 leading-relaxed text-ink-6">{content.intro}</p>

              {content.sections.map((section) => (
                <section key={section.heading} className="mb-10">
                  <h2 className="mb-3 text-xl font-semibold text-ink">{section.heading}</h2>
                  {section.paragraphs.map((paragraph) => (
                    <p key={paragraph} className="mb-3 leading-relaxed text-ink-6">
                      {paragraph}
                    </p>
                  ))}
                  {section.table && (
                    <div className="table-wrap mt-2 mb-4">
                      <table>
                        <thead>
                          <tr>
                            {section.table.headers.map((header) => (
                              <th key={header} scope="col">
                                {header}
                              </th>
                            ))}
                          </tr>
                        </thead>
                        <tbody>
                          {section.table.rows.map((row) => (
                            <tr key={row.join('|')}>
                              {row.map((cell, column) => (
                                <td key={cell} data-label={section.table!.headers[column]}>
                                  {cell}
                                </td>
                              ))}
                            </tr>
                          ))}
                        </tbody>
                      </table>
                    </div>
                  )}
                  {section.list && (
                    <ul className="list-inside list-disc space-y-1.5 leading-relaxed text-ink-6">
                      {section.list.map((item) => (
                        <li key={item}>{item}</li>
                      ))}
                    </ul>
                  )}
                  {section.afterList &&
                    section.afterList.map((paragraph) => (
                      <p key={paragraph} className="mt-3 leading-relaxed text-ink-6">
                        {paragraph}
                      </p>
                    ))}
                </section>
              ))}
            </div>
          </div>
        </main>

        <Footer />
      </div>
    </>
  );
};

export default PrivacyPolicy;
