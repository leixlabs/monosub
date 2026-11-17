import { useLanguage } from '../hooks/useLanguage';
import { CHECKOUT_URL, downloadUrlFor } from '../lib/links';
import { usePlatform } from '../hooks/usePlatform';

export const FinalCTA = () => {
  const { t } = useLanguage();
  const platform = usePlatform();
  const downloadUrl = downloadUrlFor(platform);
  const ctaLabel =
    platform === 'windows' ? t.cta.windows : platform === 'mac' ? t.cta.mac : t.cta.generic;

  return (
    <section id="cta" className="ink-band" aria-labelledby="cta-title">
      <div className="container-page section">
        <div className="max-w-3xl">
          <h2 id="cta-title" className="display-2 text-paper">
            {t.final.title}
          </h2>
          <p className="mt-5 text-[1.08rem] leading-relaxed text-[#c9c0b3] max-w-[52ch]">
            {t.final.subtitle}
          </p>
          <div className="mt-8 flex flex-wrap items-center gap-3">
            <a
              href={downloadUrl}
              target="_blank"
              rel="noopener noreferrer"
              className="btn btn-primary btn--lg"
            >
              {ctaLabel}
            </a>
            <a
              href={CHECKOUT_URL}
              target="_blank"
              rel="noopener noreferrer"
              className="btn btn-secondary btn--lg"
            >
              {t.final.cta_secondary}
            </a>
          </div>
          <ul className="mt-8 flex flex-wrap gap-x-6 gap-y-2 list-none p-0 m-0">
            {t.final.trust.map((item) => (
              <li key={item} className="flex items-center gap-2 text-sm text-[#a39a8f]">
                <span aria-hidden="true" className="inline-block w-1.5 h-1.5 rotate-45 bg-gold-500" />
                {item}
              </li>
            ))}
          </ul>
        </div>
      </div>
    </section>
  );
};
