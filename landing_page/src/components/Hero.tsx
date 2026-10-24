import { useLanguage } from '../hooks/useLanguage';
import { HeroCarousel } from './HeroCarousel';

export const Hero = () => {
  const { t } = useLanguage();

  return (
    <section className="pt-28 pb-10 px-6">
      <div className="max-w-7xl mx-auto">
        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-6">
          {/* Main headline card — full width */}
          <div className="md:col-span-2 lg:col-span-3 card p-8 md:p-12 rounded-3xl fade-in flex flex-col md:flex-row md:items-end justify-between gap-8">
            <div>
              <div className="flex flex-wrap items-center gap-3 mb-6">
                <div className="inline-block px-4 py-1.5 rounded-full bg-primary/10 border border-primary/30 text-primary text-sm font-semibold">
                  {t.hero_trust}
                </div>
                <div className="inline-block px-4 py-1.5 rounded-full bg-dark-card border border-dark-border text-gray-400 text-sm font-semibold">
                  {t.hero_platform}
                </div>
              </div>
              <h1 className="text-4xl md:text-5xl lg:text-6xl font-bold leading-tight">
                {t.hero_title_line1}
                <br />
                <span className="gradient-text">{t.hero_title_line2}</span>
                {t.hero_title_line3}
              </h1>
            </div>
            <div className="flex-shrink-0">
              <a
                href="https://github.com/leixlabs/monosub/releases/latest"
                target="_blank"
                rel="noopener noreferrer"
                className="inline-block cta-button text-white font-semibold px-8 py-4 rounded-xl text-lg transition-all"
              >
                {t.hero_cta}
              </a>
              <div className="mt-3">
                <a
                  href="#features"
                  className="text-gray-400 inline-block text-sm hover:text-primary transition-colors"
                >
                  {t.hero_learn_more}
                </a>
              </div>
            </div>
          </div>

          {/* Product screenshot card — large, prominent */}
          <div className="md:col-span-2 lg:col-span-2 lg:row-span-2 card p-4 md:p-6 rounded-3xl fade-in border-primary/40 bg-gradient-to-b from-dark-card to-dark-bg">
            <HeroCarousel />
          </div>

          {/* Badge card: local processing */}
          <div className="card p-6 md:p-8 rounded-3xl fade-in flex items-center justify-center">
            <div className="flex items-start gap-4">
              <div className="w-12 h-12 rounded-2xl bg-primary/10 border border-primary/30 flex items-center justify-center text-2xl flex-shrink-0">
                🔒
              </div>
              <div>
                <p className="font-bold text-lg leading-snug">{t.hero_subtitle_line1}</p>
              </div>
            </div>
          </div>

          {/* Badge card: completely free */}
          <div className="card p-6 md:p-8 rounded-3xl fade-in flex items-center justify-center">
            <div className="flex items-start gap-4">
              <div className="w-12 h-12 rounded-2xl bg-green-500/10 border border-green-500/30 flex items-center justify-center text-2xl flex-shrink-0">
                💰
              </div>
              <div>
                <p className="font-bold text-lg leading-snug">{t.hero_subtitle_line2}</p>
              </div>
            </div>
          </div>
        </div>
      </div>
    </section>
  );
};
