import { useEffect } from 'react';
import { Helmet, HelmetProvider } from 'react-helmet-async';
import { useLanguage } from './hooks/useLanguage';
import { Navbar } from './components/Navbar';
import { Hero } from './components/Hero';
import { PainPoints } from './components/PainPoints';
import { Features } from './components/Features';
import { Pricing } from './components/Pricing';
import { TechSpecs } from './components/TechSpecs';
import { FAQ } from './components/FAQ';
import { FinalCTA } from './components/FinalCTA';
import { Footer } from './components/Footer';

function App() {
  const { lang, t, isRTL } = useLanguage();

  useEffect(() => {
    // Set HTML lang and dir attributes
    document.documentElement.lang = lang;
    document.documentElement.dir = isRTL ? 'rtl' : 'ltr';
  }, [lang, isRTL]);

  return (
    <HelmetProvider>
      <Helmet>
        <title>{t.page_title}</title>
        <meta name="description" content={t.hero_subtitle_line1 + ' - ' + t.hero_subtitle_line2} />
        
        {/* Canonical */}
        <link rel="canonical" href={`https://monosub.com/${lang}/`} />
        
        {/* Hreflang tags */}
        <link rel="alternate" hrefLang="zh-CN" href="https://monosub.com/zh/" />
        <link rel="alternate" hrefLang="en" href="https://monosub.com/en/" />
        <link rel="alternate" hrefLang="es" href="https://monosub.com/es/" />
        <link rel="alternate" hrefLang="ru" href="https://monosub.com/ru/" />
        <link rel="alternate" hrefLang="pt-BR" href="https://monosub.com/pt-br/" />
        <link rel="alternate" hrefLang="ja" href="https://monosub.com/ja/" />
        <link rel="alternate" hrefLang="ko" href="https://monosub.com/ko/" />
        <link rel="alternate" hrefLang="de" href="https://monosub.com/de/" />
        <link rel="alternate" hrefLang="fr" href="https://monosub.com/fr/" />
        <link rel="alternate" hrefLang="zh-TW" href="https://monosub.com/zh-tw/" />
        <link rel="alternate" hrefLang="es-419" href="https://monosub.com/es-419/" />
        <link rel="alternate" hrefLang="id" href="https://monosub.com/id/" />
        <link rel="alternate" hrefLang="cs" href="https://monosub.com/cs/" />
        <link rel="alternate" hrefLang="it" href="https://monosub.com/it/" />
        <link rel="alternate" hrefLang="nl" href="https://monosub.com/nl/" />
        <link rel="alternate" hrefLang="pt-PT" href="https://monosub.com/pt-pt/" />
        <link rel="alternate" hrefLang="pl" href="https://monosub.com/pl/" />
        <link rel="alternate" hrefLang="vi" href="https://monosub.com/vi/" />
        <link rel="alternate" hrefLang="tr" href="https://monosub.com/tr/" />
        <link rel="alternate" hrefLang="uk" href="https://monosub.com/uk/" />
        <link rel="alternate" hrefLang="ar" href="https://monosub.com/ar/" />
        <link rel="alternate" hrefLang="fa" href="https://monosub.com/fa/" />
        <link rel="alternate" hrefLang="th" href="https://monosub.com/th/" />
        <link rel="alternate" hrefLang="hi" href="https://monosub.com/hi/" />
        <link rel="alternate" hrefLang="x-default" href="https://monosub.com/en/" />
        
        {/* Open Graph */}
        <meta property="og:title" content={t.page_title} />
        <meta property="og:description" content={t.hero_subtitle_line1} />
        <meta property="og:type" content="website" />
        <meta property="og:url" content={`https://monosub.com/${lang}/`} />
        
        {/* Twitter Card */}
        <meta name="twitter:card" content="summary_large_image" />
        <meta name="twitter:title" content={t.page_title} />
        <meta name="twitter:description" content={t.hero_subtitle_line1} />
      </Helmet>

      <div className="min-h-screen bg-dark-bg text-white">
        <Navbar />
        <Hero />
        <PainPoints />
        <section id="features">
          <Features />
        </section>
        <Pricing />
        <TechSpecs />
        <FAQ />
        <FinalCTA />
        <Footer />
      </div>
    </HelmetProvider>
  );
}

export default App;
