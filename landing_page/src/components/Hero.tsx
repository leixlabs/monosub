import { useLanguage } from '../hooks/useLanguage';

export const Hero = () => {
  const { t, config } = useLanguage();

  return (
    <section className="min-h-screen flex items-center justify-center px-6 py-20">
      <div className="max-w-7xl mx-auto w-full">
        <div className="grid md:grid-cols-2 gap-12 items-center">
          {/* Left: Text Content */}
          <div className="space-y-8">
            <h1 className="text-5xl md:text-7xl font-bold leading-tight">
              {t.hero_title_line1}
              <br />
              <span className="gradient-text">{t.hero_title_line2}</span>
              {t.hero_title_line3}
            </h1>
            
            <div className="space-y-3 text-xl md:text-2xl text-gray-400">
              <p>{t.hero_subtitle_line1}</p>
              <p>{t.hero_subtitle_line2}</p>
            </div>
            
            <div className="space-y-4">
              <a
                href="#cta"
                className="inline-block cta-button text-white font-semibold px-8 py-4 rounded-lg text-lg transition-all"
              >
                {t.hero_cta.replace(config.price, config.price)}
              </a>
              
              <div className="flex items-center gap-3 text-gray-400">
                <svg className="w-5 h-5 text-green-500" fill="currentColor" viewBox="0 0 20 20">
                  <path fillRule="evenodd" d="M10 18a8 8 0 100-16 8 8 0 000 16zm3.707-9.293a1 1 0 00-1.414-1.414L9 10.586 7.707 9.293a1 1 0 00-1.414 1.414l2 2a1 1 0 001.414 0l4-4z" clipRule="evenodd" />
                </svg>
                <span>{t.hero_trust}</span>
              </div>
            </div>
          </div>
          
          {/* Right: Hero Image */}
          <div className="relative">
            <div className="hero-image rounded-2xl overflow-hidden shadow-2xl border border-dark-border">
              <img
                src="/imgs/hero-mockup.png"
                alt="MonoSub Interface"
                className="w-full h-auto"
                loading="eager"
              />
            </div>
          </div>
        </div>
      </div>
    </section>
  );
};
