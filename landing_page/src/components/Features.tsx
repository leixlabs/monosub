import { useLanguage } from '../hooks/useLanguage';

export const Features = () => {
  const { t } = useLanguage();

  const features = [
    { title: t.feature_1_title, desc: t.feature_1_desc },
    { title: t.feature_2_title, desc: t.feature_2_desc },
    { title: t.feature_3_title, desc: t.feature_3_desc },
    { title: t.feature_4_title, desc: t.feature_4_desc },
    { title: t.feature_5_title, desc: t.feature_5_desc },
    { title: t.feature_6_title, desc: t.feature_6_desc },
    { title: '💎 Pro Feature: Speaker Diarization', desc: 'Identify who spoke when in your recordings, perfect for interviews, meetings, and podcasts.' },
    { title: '🚀 Pro Feature: Priority Support', desc: 'Get faster response times and dedicated support for any issues or questions you might have.' }
  ];

  return (
    <section className="py-20 px-6">
      <div className="max-w-6xl mx-auto">
        <div className="text-center mb-16 fade-in">
          <h2 className="text-4xl md:text-5xl font-bold mb-4">{t.features_title}</h2>
          <p className="text-xl text-gray-400">{t.features_subtitle}</p>
        </div>

        <div className="grid md:grid-cols-2 lg:grid-cols-3 gap-8">
          {features.map((feature, idx) => (
            <div key={idx} className="card p-6 rounded-2xl fade-in" style={{ animationDelay: `${idx * 100}ms` }}>
              <h3 className="text-xl font-bold mb-3">{feature.title}</h3>
              <p className="text-gray-400">{feature.desc}</p>
            </div>
          ))}
        </div>
      </div>
    </section>
  );
};
