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
        
        {/* Hreflang tags — only the two shipped locales are advertised */}
        <link rel="alternate" hrefLang="zh-CN" href="https://monosub.com/zh/" />
        <link rel="alternate" hrefLang="zh" href="https://monosub.com/zh/" />
        <link rel="alternate" hrefLang="en" href="https://monosub.com/en/" />
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
