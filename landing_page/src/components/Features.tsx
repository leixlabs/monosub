import { useLanguage } from '../hooks/useLanguage';

export const Features = () => {
  const { t } = useLanguage();

  return (
    <section id="features" className="section scroll-mt-24">
      <div className="container-page">
        <div className="section-head">
          <p className="eyebrow">{t.features.eyebrow}</p>
          <h2 className="display-2 mt-5">{t.features.title}</h2>
          <p className="lede mt-5" style={{ maxWidth: '54ch' }}>{t.features.subtitle}</p>
        </div>

        <ul className="mt-12 grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-px bg-line border border-line rounded overflow-hidden list-none p-0 m-0">
          {t.features.items.map((item, index) => (
            <li key={item.title} className="bg-paper p-6 md:p-7">
              <span className="index-num">{String(index + 1).padStart(2, '0')}</span>
              <h3 className="font-display text-lg mt-3 leading-snug">{item.title}</h3>
              <p className="mt-2 text-sm leading-relaxed text-ink-7">{item.desc}</p>
            </li>
          ))}
        </ul>
      </div>
    </section>
  );
};
