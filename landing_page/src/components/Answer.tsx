import { useLanguage } from '../hooks/useLanguage';

/**
 * The "short answer" band: the product as the answer to the page's core question.
 * A single dark-ink band gives the long paper page a strong beat and sets the
 * numbers apart. Gold becomes the 500 step here, as lei-brand requires on ink.
 */
export const Answer = () => {
  const { t } = useLanguage();

  return (
    <section className="ink-band" aria-labelledby="answer-title">
      <div className="container-page section--tight">
        <div className="grid lg:grid-cols-12 gap-8 lg:gap-12">
          <div className="lg:col-span-4">
            <p className="eyebrow">{t.answer.eyebrow}</p>
            <h2 id="answer-title" className="display-3 mt-4 max-w-[16ch]">
              {t.answer.title}
            </h2>
          </div>
          <div className="lg:col-span-8">
            <p className="text-[1.06rem] md:text-[1.12rem] leading-[1.72] text-[#e7dfd2] max-w-[62ch]">
              {t.answer.body}
            </p>
            <dl className="mt-10 grid grid-cols-1 sm:grid-cols-3 gap-x-8 gap-y-6 border-t border-[#332c24] pt-8">
              {t.answer.stats.map((stat) => (
                <div key={stat.label}>
                  <dt className="sr-only">{stat.label}</dt>
                  <dd className="m-0">
                    <span className="font-display text-[2.6rem] leading-none text-paper">{stat.value}</span>
                    <span className="mt-3 block text-sm leading-snug text-[#a39a8f]">{stat.label}</span>
                  </dd>
                </div>
              ))}
            </dl>
          </div>
        </div>
      </div>
    </section>
  );
};
