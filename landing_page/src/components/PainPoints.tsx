import { useLanguage } from '../hooks/useLanguage';

export const PainPoints = () => {
  const { t } = useLanguage();

  return (
    <section className="py-20 px-6">
      <div className="max-w-6xl mx-auto">
        <div className="text-center mb-16 fade-in">
          <h2 className="text-4xl md:text-5xl font-bold mb-4">{t.pain_title}</h2>
          <p className="text-xl text-gray-400">{t.pain_subtitle}</p>
        </div>

        <div className="grid md:grid-cols-2 gap-8">
          {/* Cloud Subscription (Bad) */}
          <div className="card p-8 rounded-2xl comparison-bad fade-in">
            <h3 className="text-2xl font-bold mb-6">{t.pain_cloud_title}</h3>
            <ul className="space-y-4">
              {[t.pain_cloud_1, t.pain_cloud_2, t.pain_cloud_3, t.pain_cloud_4].map((item, idx) => (
                <li key={idx} className="flex items-start gap-3">
                  <span className="text-red-500 mt-1">✗</span>
                  <span className="text-gray-300">{item}</span>
                </li>
              ))}
            </ul>
          </div>

          {/* MonoSub Solution (Good) */}
          <div className="card p-8 rounded-2xl comparison-good fade-in">
            <h3 className="text-2xl font-bold mb-6">{t.pain_local_title}</h3>
            <ul className="space-y-4">
              {[t.pain_local_1, t.pain_local_2, t.pain_local_3, t.pain_local_4].map((item, idx) => (
                <li key={idx} className="flex items-start gap-3">
                  <span className="text-green-500 mt-1">✓</span>
                  <span className="text-gray-300">{item}</span>
                </li>
              ))}
            </ul>
          </div>
        </div>
      </div>
    </section>
  );
};
