import { useLanguage } from '../hooks/useLanguage';

export const FinalCTA = () => {
  const { t } = useLanguage();

  return (
    <section id="cta" className="py-20 px-6">
      <div className="max-w-4xl mx-auto text-center">
        <div className="card p-12 rounded-3xl fade-in">
          <h2 className="text-4xl md:text-5xl font-bold mb-6">{t.final_title}</h2>
          <p className="text-xl text-gray-400 mb-8">{t.final_subtitle}</p>
          
          <a
            href="https://monosub.gumroad.com/l/monosub"
            target="_blank"
            rel="noopener noreferrer"
            className="inline-block cta-button text-white font-semibold px-10 py-5 rounded-lg text-xl transition-all mb-8"
          >
            {t.final_cta}
          </a>
          
          <div className="flex flex-wrap items-center justify-center gap-6 text-sm text-gray-400">
            <div className="flex items-center gap-2">
              <svg className="w-5 h-5 text-green-500" fill="currentColor" viewBox="0 0 20 20">
                <path fillRule="evenodd" d="M10 18a8 8 0 100-16 8 8 0 000 16zm3.707-9.293a1 1 0 00-1.414-1.414L9 10.586 7.707 9.293a1 1 0 00-1.414 1.414l2 2a1 1 0 001.414 0l4-4z" clipRule="evenodd" />
              </svg>
              <span>{t.final_trust_1}</span>
            </div>
            <div className="flex items-center gap-2">
              <svg className="w-5 h-5 text-green-500" fill="currentColor" viewBox="0 0 20 20">
                <path fillRule="evenodd" d="M10 18a8 8 0 100-16 8 8 0 000 16zm3.707-9.293a1 1 0 00-1.414-1.414L9 10.586 7.707 9.293a1 1 0 00-1.414 1.414l2 2a1 1 0 001.414 0l4-4z" clipRule="evenodd" />
              </svg>
              <span>{t.final_trust_2}</span>
            </div>
            <div className="flex items-center gap-2">
              <svg className="w-5 h-5 text-green-500" fill="currentColor" viewBox="0 0 20 20">
                <path fillRule="evenodd" d="M10 18a8 8 0 100-16 8 8 0 000 16zm3.707-9.293a1 1 0 00-1.414-1.414L9 10.586 7.707 9.293a1 1 0 00-1.414 1.414l2 2a1 1 0 001.414 0l4-4z" clipRule="evenodd" />
              </svg>
              <span>{t.final_trust_3}</span>
            </div>
          </div>
          
          <p className="text-sm text-gray-500 mt-6">{t.final_payment}</p>
        </div>
      </div>
    </section>
  );
};
