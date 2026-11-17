import { useLanguage } from '../hooks/useLanguage';

export const WhyLocal = () => {
  const { t } = useLanguage();

  return (
    <section id="why" className="section scroll-mt-24">
      <div className="container-page">
        <div className="grid lg:grid-cols-12 gap-8 lg:gap-12 items-start">
          <div className="lg:col-span-5">
            <p className="eyebrow">{t.why.eyebrow}</p>
            <h2 className="display-2 mt-5">{t.why.q}</h2>
          </div>
          <div className="lg:col-span-7">
            <p className="text-[1.05rem] leading-[1.75] text-ink-8 max-w-[60ch]">{t.why.a}</p>
          </div>
        </div>

        <div className="mt-12 grid md:grid-cols-2 gap-5 lg:gap-6">
          {/* Cloud */}
          <div className="card card--flat p-7 md:p-8">
            <h3 className="font-display text-xl text-ink-6">{t.why.cloud_title}</h3>
            <ul className="mt-5 list-none p-0 m-0 grid gap-3.5">
              {t.why.cloud.map((item) => (
                <li key={item} className="flex items-start gap-3 text-ink-7">
                  <svg className="cross mt-[0.35rem]" width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.7" aria-hidden="true">
                    <path d="M4 4l8 8M12 4l-8 8" strokeLinecap="round" />
                  </svg>
                  <span>{item}</span>
                  <span className="sr-only"> (drawback)</span>
                </li>
              ))}
            </ul>
          </div>

          {/* MonoSub */}
          <div className="card p-7 md:p-8">
            <h3 className="font-display text-xl text-ink">{t.why.local_title}</h3>
            <ul className="mt-5 list-none p-0 m-0 grid gap-3.5">
              {t.why.local.map((item) => (
                <li key={item} className="flex items-start gap-3 text-ink-8">
                  <svg className="tick mt-[0.35rem]" width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.9" aria-hidden="true">
                    <path d="M2.5 8.5l3.5 3.5 7.5-8" strokeLinecap="round" strokeLinejoin="round" />
                  </svg>
                  <span>{item}</span>
                  <span className="sr-only"> (benefit)</span>
                </li>
              ))}
            </ul>
          </div>
        </div>
      </div>
    </section>
  );
};
