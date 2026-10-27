import { useLanguage } from '../hooks/useLanguage';
import { STORE_URL, RELEASES_URL } from '../lib/links';

export const FinalCTA = () => {
  const { t } = useLanguage();

  // Inline SVG rather than emoji: emoji render in whatever palette the platform
  // chose, and this page has a strict no-blue palette.
  const trust = [
    { path: 'M2 7h20v10H2z M2 10h20', text: t.final_trust_1 },
    { path: 'M4 12a8 8 0 1 1 8 8', text: t.final_trust_2 },
    { path: 'M3 4h18v12H3z M8 20h8', text: t.final_trust_3 }
  ];

  return (
    <section id="cta" className="py-16 px-6">
      <div className="max-w-7xl mx-auto">
        <div className="card p-10 md:p-16 rounded-3xl fade-in text-center relative overflow-hidden">
          {/* Decorative glow */}
          <div className="absolute -top-24 left-1/2 -translate-x-1/2 w-96 h-96 bg-primary/20 rounded-full blur-3xl pointer-events-none" />

          <div className="relative">
            <h2 className="text-4xl md:text-5xl font-bold mb-6">{t.final_title}</h2>
            <p className="text-xl text-gray-400 mb-10 max-w-2xl mx-auto">{t.final_subtitle}</p>

            <a
              href={STORE_URL}
              target="_blank"
              rel="noopener noreferrer"
              className="inline-block cta-button text-white font-semibold px-10 py-5 rounded-xl text-xl transition-all"
            >
              {t.final_cta}
            </a>

            <div className="mt-5">
              <a
                href={RELEASES_URL}
                target="_blank"
                rel="noopener noreferrer"
                className="text-gray-400 text-sm hover:text-primary transition-colors"
              >
                {t.hero_cta_mac} →
              </a>
            </div>

            <div className="flex flex-wrap items-center justify-center gap-4 mt-10">
              {trust.map((item, idx) => (
                <div
                  key={idx}
                  className="flex items-center gap-2 px-5 py-3 rounded-2xl bg-dark-card border border-dark-border text-sm text-gray-400"
                >
                  <svg
                    className="w-5 h-5 text-primary flex-shrink-0"
                    fill="none"
                    stroke="currentColor"
                    strokeWidth={1.6}
                    strokeLinecap="round"
                    strokeLinejoin="round"
                    viewBox="0 0 24 24"
                    aria-hidden="true"
                  >
                    <path d={item.path} />
                  </svg>
                  <span>{item.text}</span>
                </div>
              ))}
            </div>
          </div>
        </div>
      </div>
    </section>
  );
};
