import { useEffect } from 'react';
import { Helmet } from 'react-helmet-async';
import { useLanguage } from './hooks/useLanguage';
import { Navbar } from './components/Navbar';
import { Hero } from './components/Hero';
import { Answer } from './components/Answer';
import { WhyLocal } from './components/WhyLocal';
import { HowItWorks } from './components/HowItWorks';
import { Features } from './components/Features';
import { Pricing } from './components/Pricing';
import { FAQ } from './components/FAQ';
import { FinalCTA } from './components/FinalCTA';
import { Footer } from './components/Footer';
import { alternateLinks, pageUrl } from './lib/seo';
import { buildStructuredData } from './lib/structuredData';
import { absoluteUrl } from './lib/site';

function App() {
  const { lang, t, isRTL } = useLanguage();

  useEffect(() => {
    document.documentElement.lang = lang;
    document.documentElement.dir = isRTL ? 'rtl' : 'ltr';
  }, [lang, isRTL]);

  const canonical = pageUrl(lang, 'home');
  const description = t.page_description;
  const jsonLd = buildStructuredData(lang, t, `/${lang}/`);

  return (
    <>
      <Helmet>
        <html lang={lang} dir={isRTL ? 'rtl' : 'ltr'} />
        <title>{t.page_title}</title>
        <meta name="description" content={description} />

        {/* Canonical */}
        <link rel="canonical" href={canonical} />

        {/* Hreflang — only the two shipped locales are advertised */}
        {alternateLinks('home').map((link) => (
          <link key={link.hrefLang} rel="alternate" hrefLang={link.hrefLang} href={link.href} />
        ))}

        {/* Open Graph */}
        <meta property="og:title" content={t.page_title} />
        <meta property="og:description" content={description} />
        <meta property="og:type" content="website" />
        <meta property="og:url" content={canonical} />
        <meta property="og:image" content={absoluteUrl('/og-image.jpg')} />
        <meta property="og:image:width" content="1200" />
        <meta property="og:image:height" content="630" />
        <meta property="og:image:alt" content={t.og_alt} />
        <meta property="og:site_name" content="MonoSub" />
        <meta property="og:locale" content={lang === 'zh' ? 'zh_CN' : 'en_US'} />

        {/* Twitter Card */}
        <meta name="twitter:card" content="summary_large_image" />
        <meta name="twitter:title" content={t.page_title} />
        <meta name="twitter:description" content={description} />
        <meta name="twitter:image" content={absoluteUrl('/og-image.jpg')} />

        {/* Icons */}
        <link rel="icon" type="image/png" href="/icon.png" />
        <link rel="apple-touch-icon" href="/icon.png" />

        {/*
          JSON-LD. Rendered through Helmet so the prerenderer bakes it into the
          static HTML — crawlers that never run JS (Baidu among them) can only
          see what is in the served document.
        */}
        <script type="application/ld+json">{JSON.stringify(jsonLd)}</script>
      </Helmet>

      <div className="min-h-screen">
        <Navbar />
        <main>
          <Hero />
          <Answer />
          <WhyLocal />
          <HowItWorks />
          <Features />
          <Pricing />
          <FAQ />
          <FinalCTA />
        </main>
        <Footer />
      </div>
    </>
  );
}

export default App;
