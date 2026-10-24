import { useEffect } from 'react';
import { Helmet, HelmetProvider } from 'react-helmet-async';
import { useLanguage } from './hooks/useLanguage';
import { Navbar } from './components/Navbar';
import { Hero } from './components/Hero';
import { Showcase } from './components/Showcase';
import { PainPoints } from './components/PainPoints';
import { Features } from './components/Features';
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
        <link rel="canonical" href={`https://monosub.leix.dev/${lang}/`} />
        
        {/* Hreflang tags — only the two shipped locales are advertised */}
        <link rel="alternate" hrefLang="zh-CN" href="https://monosub.leix.dev/zh/" />
        <link rel="alternate" hrefLang="zh" href="https://monosub.leix.dev/zh/" />
        <link rel="alternate" hrefLang="en" href="https://monosub.leix.dev/en/" />
        <link rel="alternate" hrefLang="x-default" href="https://monosub.leix.dev/en/" />
        
        {/* Open Graph */}
        <meta property="og:title" content={t.page_title} />
        <meta property="og:description" content={t.hero_subtitle_line1} />
        <meta property="og:type" content="website" />
        <meta property="og:url" content={`https://monosub.leix.dev/${lang}/`} />
        <meta property="og:image" content="https://monosub.leix.dev/og-image.jpg" />
        <meta property="og:image:width" content="1200" />
        <meta property="og:image:height" content="630" />
        <meta property="og:image:alt" content={t.hero_shot_1} />
        <meta property="og:site_name" content="MonoSub" />
        <meta property="og:locale" content={lang === 'zh' ? 'zh_CN' : 'en_US'} />

        {/* Twitter Card */}
        <meta name="twitter:card" content="summary_large_image" />
        <meta name="twitter:title" content={t.page_title} />
        <meta name="twitter:description" content={t.hero_subtitle_line1} />
        <meta name="twitter:image" content="https://monosub.leix.dev/og-image.jpg" />

        {/* Icons */}
        <link rel="icon" type="image/png" href="/icon.png" />
        <link rel="apple-touch-icon" href="/icon.png" />
      </Helmet>

      <div className="min-h-screen bg-dark-bg text-white">
        <Navbar />
        <Hero />
        <Showcase />
        <PainPoints />
        <Features />
        <FAQ />
        <FinalCTA />
        <Footer />
      </div>
    </HelmetProvider>
  );
}

export default App;
