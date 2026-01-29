import { useLanguage } from '../hooks/useLanguage';

export const Pricing = () => {
  const { t } = useLanguage();

  const rows = [
    { period: t.pricing_1month, cloud: t.pricing_cloud_1month, monosub: '' },
    { period: t.pricing_6months, cloud: t.pricing_cloud_6months, monosub: '' },
    { period: t.pricing_1year, cloud: t.pricing_cloud_1year, monosub: '' },
    { period: t.pricing_2years, cloud: t.pricing_cloud_2years, monosub: '' },
    { period: t.pricing_5years, cloud: t.pricing_cloud_5years, monosub: '' }
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
                <th className="pb-4 pr-4 font-bold">{t.pricing_period}</th>
                <th className="pb-4 px-4 font-bold text-red-400">{t.pricing_cloud}</th>
                <th className="pb-4 pl-4 font-bold text-green-400">{t.pricing_monosub}</th>
              </tr>
            </thead>
            <tbody>
              {rows.map((row, idx) => (
                <tr key={idx} className="border-b border-dark-border last:border-0">
                  <td className="py-4 pr-4">{row.period}</td>
                  <td className="py-4 px-4 text-gray-400">{row.cloud}</td>
                  <td className="py-4 pl-4">
                    {idx === 0 ? (
                      <span className="text-2xl font-bold gradient-text">{t.pricing_monosub_price}</span>
                    ) : (
                      <span className="text-gray-600">—</span>
                    )}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>

          <div className="mt-8 p-6 bg-primary/10 rounded-xl border border-primary/20">
            <p className="text-lg mb-2">{t.pricing_cta}</p>
            <p className="text-sm text-gray-400">{t.pricing_save}</p>
          </div>
        </div>
      </div>
    </section>
  );
};
