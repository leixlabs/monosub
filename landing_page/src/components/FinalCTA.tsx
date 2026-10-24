import { useLanguage } from '../hooks/useLanguage';

export const FinalCTA = () => {
  const { t } = useLanguage();

  const trust = [
    { icon: '🛡️', text: t.final_trust_1 },
    { icon: '🔑', text: t.final_trust_2 },
    { icon: '⚡', text: t.final_trust_3 }
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
              href="https://github.com/leixlabs/monosub/releases/latest"
              target="_blank"
              rel="noopener noreferrer"
              className="inline-block cta-button text-white font-semibold px-10 py-5 rounded-xl text-xl transition-all mb-10"
            >
              {t.final_cta}
            </a>

            <div className="flex flex-wrap items-center justify-center gap-4">
              {trust.map((item, idx) => (
                <div
                  key={idx}
                  className="flex items-center gap-2 px-5 py-3 rounded-2xl bg-dark-card border border-dark-border text-sm text-gray-400"
                >
                  <span className="text-lg">{item.icon}</span>
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
