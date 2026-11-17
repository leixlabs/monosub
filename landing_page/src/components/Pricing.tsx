import { useLanguage } from '../hooks/useLanguage';
import {
  APP_STORE_URL,
  CHECKOUT_URL,
  MS_STORE_URL,
  REPO_URL,
} from '../lib/links';
import type { ChannelId } from '../i18n';

const CHANNEL_HREF: Record<ChannelId, string> = {
  appstore: APP_STORE_URL,
  direct: CHECKOUT_URL,
  windows: MS_STORE_URL,
  source: REPO_URL,
};

export const Pricing = () => {
  const { t } = useLanguage();
  const plans = t.pricing.plans;

  return (
    <section id="pricing" className="section scroll-mt-24 border-t border-line bg-paper-2">
      <div className="container-page">
        <div className="section-head section-head--center">
          <p className="eyebrow">{t.pricing.eyebrow}</p>
          <h2 className="display-2 mt-5">{t.pricing.title}</h2>
          <p className="lede mt-5 mx-auto" style={{ maxWidth: '50ch' }}>{t.pricing.subtitle}</p>
        </div>

        {/* Plans */}
        <div className="mt-12 grid md:grid-cols-2 gap-5 lg:gap-6 max-w-4xl mx-auto">
          {plans.map((plan) => {
            const featured = plan.id === 'pro';
            return (
              <div
                key={plan.id}
                className={`card p-7 md:p-8 flex flex-col ${featured ? 'border-gold/60 shadow-[0_20px_60px_-30px_rgba(150,89,0,0.5)]' : ''}`}
              >
                <div className="flex items-center justify-between gap-3">
                  <h3 className="font-display text-xl">{plan.name}</h3>
                  {plan.badge && (
                    <span className={`badge ${featured ? 'badge--gold' : ''}`}>{plan.badge}</span>
                  )}
                </div>
                <p className="mt-4 text-sm text-ink-6">{plan.summary}</p>
                <p className="mt-5 flex items-baseline gap-2">
                  <span className="font-display text-4xl text-ink">{plan.price}</span>
                </p>
                <p className="mt-1 text-xs text-ink-6 min-h-[2.5em]">{plan.priceNote}</p>

                <ul className="mt-6 mb-7 list-none p-0 m-0 grid gap-2.5 flex-1">
                  {plan.features.map((feature) => (
                    <li key={feature} className="flex items-start gap-2.5 text-sm text-ink-8">
                      <svg className="tick mt-0.5" width="15" height="15" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.9" aria-hidden="true">
                        <path d="M2.5 8.5l3.5 3.5 7.5-8" strokeLinecap="round" strokeLinejoin="round" />
                      </svg>
                      {feature}
                    </li>
                  ))}
                </ul>

                <a
                  href={featured ? CHECKOUT_URL : APP_STORE_URL}
                  target="_blank"
                  rel="noopener noreferrer"
                  className={`btn btn-block ${featured ? 'btn-primary' : 'btn-secondary'}`}
                >
                  {plan.cta}
                </a>
              </div>
            );
          })}
        </div>

        {/* What Pro buys */}
        <div className="mt-10 max-w-4xl mx-auto rounded border-l-2 border-gold bg-gold-200/60 px-6 py-5">
          <h3 className="font-display text-lg text-ink">{t.pricing.proNoteTitle}</h3>
          <p className="mt-2 text-ink-7 leading-relaxed text-sm">{t.pricing.proNote}</p>
        </div>

        {/* Channels */}
        <div className="mt-14 max-w-4xl mx-auto">
          <h3 className="font-display text-2xl text-ink">{t.pricing.channelsTitle}</h3>

          <div className="table-wrap mt-5">
            <table>
              <caption className="sr-only">{t.pricing.channelsTitle}</caption>
              <thead>
                <tr>
                  <th scope="col">{t.pricing.columns.platform}</th>
                  <th scope="col">{t.pricing.columns.price}</th>
                  <th scope="col">{t.pricing.columns.pro}</th>
                </tr>
              </thead>
              <tbody>
                {t.pricing.channels.map((channel) => (
                  <tr key={channel.id}>
                    <th scope="row">
                      <a
                        href={CHANNEL_HREF[channel.id]}
                        target="_blank"
                        rel="noopener noreferrer"
                        className="link-gold"
                      >
                        {channel.platform}
                      </a>
                      <span className="mt-1 block text-xs font-normal text-ink-6">{channel.note}</span>
                    </th>
                    <td data-label={t.pricing.columns.price}>{channel.price}</td>
                    <td data-label={t.pricing.columns.pro}>{channel.pro}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
          <p className="mt-4 text-xs leading-relaxed text-ink-6">{t.pricing.channelsNote}</p>
        </div>
      </div>
    </section>
  );
};
