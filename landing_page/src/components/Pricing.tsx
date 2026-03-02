import { useLanguage } from '../hooks/useLanguage';

export const Pricing = () => {
  const { t } = useLanguage();

  const features = [
    { feature: t.pricing_feature_transcription, free: t.pricing_check, pro: t.pricing_check },
    { feature: t.pricing_feature_export, free: t.pricing_check, pro: t.pricing_check },
    { feature: t.pricing_feature_batch, free: t.pricing_check, pro: t.pricing_check },
    { feature: t.pricing_feature_speaker, free: t.pricing_x, pro: t.pricing_check },
    { feature: t.pricing_feature_support, free: t.pricing_x, pro: t.pricing_check }
  ];

  return (
    <section id="pricing" className="py-20 px-6">
      <div className="max-w-5xl mx-auto">
        <div className="text-center mb-16 fade-in">
          <h2 className="text-4xl md:text-5xl font-bold mb-4">{t.pricing_title}</h2>
          <p className="text-xl text-gray-400">{t.pricing_subtitle}</p>
        </div>

        <div className="card p-8 rounded-2xl overflow-x-auto fade-in">
          <table className="w-full text-left">
            <thead>
              <tr className="border-b border-dark-border">
                <th className="pb-4 pr-4 font-bold">{t.pricing_feature}</th>
                <th className="pb-4 px-4 font-bold text-gray-400">{t.pricing_free}</th>
                <th className="pb-4 pl-4 font-bold text-green-400">{t.pricing_pro}</th>
              </tr>
            </thead>
            <tbody>
              {features.map((item, idx) => (
                <tr key={idx} className="border-b border-dark-border last:border-0">
                  <td className="py-4 pr-4">{item.feature}</td>
                  <td className="py-4 px-4">{item.free}</td>
                  <td className="py-4 pl-4">{item.pro}</td>
                </tr>
              ))}
            </tbody>
          </table>

          <div className="mt-12 text-center">
            <div className="inline-block p-6 bg-primary/10 rounded-xl border border-primary/20">
              <p className="text-2xl font-bold mb-2">{t.pricing_pro_price}</p>
              <p className="text-lg mb-4">{t.pricing_pro_description}</p>
              <p className="text-sm text-gray-400 mb-6">{t.pricing_pro_discount}</p>
              <a
                href="https://polar.leix.dev/v1/checkout-links/polar_cl_JgYlmsUnlbej0vpnuI197nQltlEsUxg1hZd0r3unixT/redirect"
                target="_blank"
                rel="noopener noreferrer"
                className="inline-block cta-button text-white font-semibold px-8 py-4 rounded-lg text-lg transition-all"
              >
                {t.pricing_pro_cta}
              </a>
            </div>
          </div>
        </div>
      </div>
    </section>
  );
};
