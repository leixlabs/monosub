import { useLanguage } from '../hooks/useLanguage';

export const Features = () => {
  const { t } = useLanguage();

  // A four-column grid: two rows of wide cards, then two rows of pairs. Every
  // row adds up to four columns, so no cell is left with dead space.
  const features = [
    { title: t.feature_1_title, desc: t.feature_1_desc, span: 'lg:col-span-2', highlight: false },
    { title: t.feature_2_title, desc: t.feature_2_desc, span: 'lg:col-span-2', highlight: true },
    { title: t.feature_3_title, desc: t.feature_3_desc, span: 'lg:col-span-1', highlight: false },
    { title: t.feature_4_title, desc: t.feature_4_desc, span: 'lg:col-span-1', highlight: false },
    { title: t.feature_7_title, desc: t.feature_7_desc, span: 'lg:col-span-1', highlight: false },
    { title: t.feature_8_title, desc: t.feature_8_desc, span: 'lg:col-span-1', highlight: false },
    { title: t.feature_5_title, desc: t.feature_5_desc, span: 'lg:col-span-2', highlight: false },
    { title: t.feature_6_title, desc: t.feature_6_desc, span: 'lg:col-span-2', highlight: false }
  ];

  return (
    <section id="features" className="py-16 px-6 scroll-mt-24">
      <div className="max-w-7xl mx-auto">
        <div className="text-center mb-12 fade-in">
          <h2 className="text-4xl md:text-5xl font-bold mb-4">{t.features_title}</h2>
          <p className="text-xl text-gray-400">{t.features_subtitle}</p>
        </div>

        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-6">
          {features.map((feature, idx) => (
            <div
              key={idx}
              className={`card p-6 rounded-3xl fade-in ${feature.span} ${
                feature.highlight
                  ? 'bg-gradient-to-br from-primary/15 via-dark-card to-dark-card border-primary/40'
                  : ''
              }`}
              style={{ animationDelay: `${idx * 80}ms` }}
            >
              <h3 className={`font-bold mb-3 ${feature.highlight ? 'text-2xl' : 'text-xl'}`}>
                {feature.title}
              </h3>
              <p className={`text-gray-400 ${feature.highlight ? 'text-lg' : ''}`}>
                {feature.desc}
              </p>
            </div>
          ))}
        </div>
      </div>
    </section>
  );
};
