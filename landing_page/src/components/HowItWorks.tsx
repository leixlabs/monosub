import { useLanguage } from '../hooks/useLanguage';

export const HowItWorks = () => {
  const { t } = useLanguage();

  return (
    <section id="how" className="section scroll-mt-24 border-y border-line bg-paper-2">
      <div className="container-page">
        <div className="section-head">
          <p className="eyebrow">{t.how.eyebrow}</p>
          <h2 className="display-2 mt-5">{t.how.title}</h2>
          <p className="lede mt-5" style={{ maxWidth: '52ch' }}>{t.how.subtitle}</p>
        </div>

        <ol className="mt-12 grid md:grid-cols-3 gap-x-10 gap-y-10 list-none p-0 m-0">
          {t.how.steps.map((step, index) => (
            <li key={step.title} className="relative">
              <div className="flex items-baseline gap-3">
                <span className="index-num text-[0.9rem]">
                  {String(index + 1).padStart(2, '0')}
                </span>
                <span aria-hidden="true" className="flex-1 border-t border-line self-center" />
              </div>
              <h3 className="font-display text-xl mt-4">{step.title}</h3>
              <p className="mt-2.5 text-ink-7 leading-relaxed">{step.desc}</p>
            </li>
          ))}
        </ol>

        <p className="mt-12 text-sm text-ink-6 max-w-[60ch]">{t.how.note}</p>
      </div>
    </section>
  );
};
