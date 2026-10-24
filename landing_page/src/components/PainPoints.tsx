import { useLanguage } from '../hooks/useLanguage';

export const PainPoints = () => {
  const { t } = useLanguage();

  return (
    <section className="py-16 px-6">
      <div className="max-w-7xl mx-auto">
        <div className="text-center mb-12 fade-in">
          <h2 className="text-4xl md:text-5xl font-bold mb-4">{t.pain_title}</h2>
          <p className="text-xl text-gray-400">{t.pain_subtitle}</p>
        </div>

        <div className="grid md:grid-cols-2 gap-6">
          {/* Cloud Subscription (Bad) */}
          <div className="card rounded-3xl overflow-hidden fade-in">
            <div className="px-8 pt-6 pb-4 border-b border-dark-border bg-red-500/5">
              <h3 className="text-2xl font-bold">{t.pain_cloud_title}</h3>
            </div>
            <ul className="p-8 space-y-4">
              {[t.pain_cloud_1, t.pain_cloud_2, t.pain_cloud_3, t.pain_cloud_4].map((item, idx) => (
                <li key={idx} className="flex items-start gap-3">
                  <span className="text-red-500 mt-1">✗</span>
                  <span className="text-gray-300">{item}</span>
                </li>
              ))}
            </ul>
          </div>

          {/* MonoSub Solution (Good) */}
          <div className="card rounded-3xl overflow-hidden fade-in">
            <div className="px-8 pt-6 pb-4 border-b border-dark-border bg-green-500/5">
              <h3 className="text-2xl font-bold">{t.pain_local_title}</h3>
            </div>
            <ul className="p-8 space-y-4">
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
