import { useLanguage } from '../hooks/useLanguage';

/**
 * Native `<details>` accordion: no JavaScript, works before hydration, and the
 * closed answers stay findable by the browser's find-in-page (which `display:none`
 * would break). Items are independent — the platform accordion guide warns that
 * exclusive groups cause content to jump out of view for screen-magnifier users.
 */
export const FAQ = () => {
  const { t } = useLanguage();

  return (
    <section id="faq" className="section scroll-mt-24">
      <div className="container-page">
        <div className="grid lg:grid-cols-12 gap-8 lg:gap-14">
          <div className="lg:col-span-4">
            <p className="eyebrow">{t.faq.eyebrow}</p>
            <h2 className="display-2 mt-5">{t.faq.title}</h2>
            <p className="mt-4 text-ink-6 max-w-[30ch]">{t.faq.subtitle}</p>
          </div>

          <div className="lg:col-span-8">
            <div className="faq-list">
              {t.faq.items.map((item) => (
                <details key={item.q} className="faq-item">
                  <summary>{item.q}</summary>
                  <div className="faq-answer">{item.a}</div>
                </details>
              ))}
            </div>
          </div>
        </div>
      </div>
    </section>
  );
};
