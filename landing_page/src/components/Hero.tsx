import { useLanguage } from '../hooks/useLanguage';
import { APP_STORE_URL } from '../lib/links';
import { shot, shotSize, heroShot } from '../lib/screenshots';
import { highPriorityImage } from '../lib/imagePriority';

export const Hero = () => {
  const { t, lang } = useLanguage();
  const name = heroShot(lang);
  const size = shotSize(name);

  return (
    <section className="pt-14 pb-10 md:pt-20">
      <div className="container-page">
        <div className="grid lg:grid-cols-12 gap-10 lg:gap-12 items-center">
          {/* Copy */}
          <div className="lg:col-span-7">
            <p className="eyebrow">{t.hero.eyebrow}</p>
            <h1 className="display-1 mt-5">
              {t.hero.title}
              <span className="block text-ink-6 mt-1">{t.hero.title_accent}</span>
            </h1>
            <p className="lede mt-6">{t.hero.subtitle}</p>

            <div className="mt-8 flex flex-wrap items-center gap-3">
              <a
                href={APP_STORE_URL}
                target="_blank"
                rel="noopener noreferrer"
                className="btn btn-primary btn--lg"
              >
                {t.hero.cta_primary}
              </a>
              <a href="#pricing" className="btn btn-secondary btn--lg">
                {t.hero.cta_secondary}
              </a>
            </div>
            <p className="mt-4 text-sm text-ink-6">{t.hero.note}</p>

            <ul className="mt-8 flex flex-wrap gap-x-6 gap-y-2 list-none p-0 m-0">
              {t.hero.trust.map((item) => (
                <li key={item} className="flex items-center gap-2 text-sm text-ink-7">
                  <span aria-hidden="true" className="inline-block w-1.5 h-1.5 rotate-45 bg-gold" />
                  {item}
                </li>
              ))}
            </ul>
          </div>

          {/* Screenshot */}
          <div className="lg:col-span-5">
            <div className="shot-frame">
              <img
                src={shot(name)}
                alt={t.hero.shot_caption}
                width={size.width}
                height={size.height}
                loading="eager"
                decoding="async"
                {...highPriorityImage}
              />
            </div>
            <p className="mt-3 text-xs text-ink-6 text-center">{t.hero.shot_caption}</p>
          </div>
        </div>
      </div>
    </section>
  );
};
