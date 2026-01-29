import { useLanguage } from '../hooks/useLanguage';

export const TechSpecs = () => {
  const { t } = useLanguage();

  return (
    <section className="py-20 px-6">
      <div className="max-w-5xl mx-auto">
        <div className="text-center mb-16 fade-in">
          <h2 className="text-4xl md:text-5xl font-bold mb-4">{t.tech_title}</h2>
          <p className="text-xl text-gray-400">{t.tech_subtitle}</p>
        </div>

        <div className="grid md:grid-cols-2 gap-8 mb-12">
          <div className="card p-6 rounded-2xl fade-in">
            <h3 className="text-lg font-bold text-gray-400 mb-2">{t.tech_model}</h3>
            <p className="text-2xl font-bold">{t.tech_model_value}</p>
          </div>
          
          <div className="card p-6 rounded-2xl fade-in">
            <h3 className="text-lg font-bold text-gray-400 mb-2">{t.tech_framework}</h3>
            <p className="text-2xl font-bold">{t.tech_framework_value}</p>
          </div>
          
          <div className="card p-6 rounded-2xl fade-in">
            <h3 className="text-lg font-bold text-gray-400 mb-2">{t.tech_acceleration}</h3>
            <p className="text-2xl font-bold">{t.tech_acceleration_value}</p>
          </div>
          
          <div className="card p-6 rounded-2xl fade-in">
            <h3 className="text-lg font-bold text-gray-400 mb-2">{t.tech_platforms}</h3>
            <div className="space-y-2 mt-3">
              <p className="text-lg">{t.tech_platform_macos}</p>
              <p className="text-lg text-gray-500">{t.tech_platform_windows}</p>
              <p className="text-lg text-gray-500">{t.tech_platform_linux}</p>
            </div>
          </div>
        </div>
      </div>
    </section>
  );
};
