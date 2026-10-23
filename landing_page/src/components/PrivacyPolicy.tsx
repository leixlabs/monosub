import { useEffect } from 'react';
import { Helmet, HelmetProvider } from 'react-helmet-async';
import { Link } from 'react-router-dom';
import { useLanguage } from '../hooks/useLanguage';
import { LanguageSwitcher } from './LanguageSwitcher';
import { Footer } from './Footer';
import { privacyContent } from '../i18n/privacy';

export const PrivacyPolicy = () => {
  const { lang } = useLanguage();
  const content = privacyContent[lang] || privacyContent.en;

  useEffect(() => {
    document.documentElement.lang = lang;
    document.documentElement.dir = 'ltr';
  }, [lang]);

  return (
    <HelmetProvider>
      <Helmet>
        <title>{content.meta_title}</title>
        <meta name="description" content={content.meta_description} />

        {/* Canonical */}
        <link rel="canonical" href={`https://monosub.com/${lang}/privacy`} />

        {/* Hreflang tags */}
        <link rel="alternate" hrefLang="zh-CN" href="https://monosub.com/zh/privacy" />
        <link rel="alternate" hrefLang="zh" href="https://monosub.com/zh/privacy" />
        <link rel="alternate" hrefLang="en" href="https://monosub.com/en/privacy" />
        <link rel="alternate" hrefLang="x-default" href="https://monosub.com/en/privacy" />

        {/* Open Graph */}
        <meta property="og:title" content={content.meta_title} />
        <meta property="og:description" content={content.meta_description} />
        <meta property="og:type" content="website" />
        <meta property="og:url" content={`https://monosub.com/${lang}/privacy`} />

        {/* Twitter Card */}
        <meta name="twitter:card" content="summary" />
        <meta name="twitter:title" content={content.meta_title} />
        <meta name="twitter:description" content={content.meta_description} />
      </Helmet>

      <div className="min-h-screen bg-dark-bg text-white">
        <nav className="fixed top-0 left-0 right-0 z-50 px-6 py-4 bg-dark-bg/80 backdrop-blur-lg border-b border-dark-border">
          <div className="max-w-7xl mx-auto flex items-center justify-between">
            <Link to={`/${lang}`} className="text-2xl font-bold gradient-text">
              MonoSub
            </Link>
            <div className="flex items-center gap-4">
              <Link
                to={`/${lang}`}
                className="hidden sm:inline text-gray-400 hover:text-white transition-colors text-sm"
              >
                {content.back_home}
              </Link>
              <LanguageSwitcher />
            </div>
          </div>
        </nav>

        <main className="max-w-3xl mx-auto px-6 pt-28 pb-16">
          <p className="text-sm text-gray-500">
            {content.last_updated_label}: {content.last_updated}
          </p>
          <h1 className="text-4xl font-bold mt-2 mb-6">{content.title}</h1>
          <p className="text-gray-400 mb-12 leading-relaxed">{content.intro}</p>

          {content.sections.map((section) => (
            <section key={section.heading} className="mb-10">
              <h2 className="text-xl font-semibold text-primary mb-3">{section.heading}</h2>
              {section.paragraphs.map((paragraph) => (
                <p key={paragraph} className="text-gray-400 leading-relaxed mb-3">
                  {paragraph}
                </p>
              ))}
              {section.list && (
                <ul className="list-disc list-inside text-gray-400 space-y-1.5 leading-relaxed">
                  {section.list.map((item) => (
                    <li key={item}>{item}</li>
                  ))}
                </ul>
              )}
              {section.afterList &&
                section.afterList.map((paragraph) => (
                  <p key={paragraph} className="text-gray-400 leading-relaxed mt-3">
                    {paragraph}
                  </p>
                ))}
            </section>
          ))}
        </main>

        <Footer />
      </div>
    </HelmetProvider>
  );
};

export default PrivacyPolicy;
