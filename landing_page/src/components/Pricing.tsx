import { useLanguage } from '../hooks/useLanguage';
import { STORE_URL, RELEASES_URL } from '../lib/links';

/**
 * Two purchase cards side by side, then the capability table underneath. The
 * table is deliberately identical on both sides except for signing and updates:
 * the point of the section is that the money buys packaging, not features.
 */
export const Pricing = () => {
  const { t } = useLanguage();

  const rows = [
    { label: t.pricing_row_transcription, store: true, mac: true },
    { label: t.pricing_row_export, store: true, mac: true },
    { label: t.pricing_row_batch, store: true, mac: true },
    { label: t.pricing_row_translate, store: true, mac: true },
    { label: t.pricing_row_speakers, store: true, mac: true },
    { label: t.pricing_row_words, store: true, mac: true },
    // The macOS build is ad-hoc signed; Gatekeeper still needs the xattr step
    // documented in the README, so it cannot claim platform verification.
    { label: t.pricing_row_signed, store: true, mac: false },
    { label: t.pricing_row_updates, store: true, mac: false },
    { label: t.pricing_row_setup, store: t.pricing_setup_store, mac: t.pricing_setup_mac }
  ];

  return (
    <section id="pricing" className="py-16 px-6 scroll-mt-24">
      <div className="max-w-6xl mx-auto">
        <div className="text-center mb-12 fade-in">
          <h2 className="text-4xl md:text-5xl font-bold mb-4">{t.pricing_title}</h2>
          <p className="text-xl text-gray-400 max-w-2xl mx-auto">{t.pricing_subtitle}</p>
        </div>

        {/* The two purchase options */}
        <div className="grid md:grid-cols-2 gap-6 mb-10">
          <div className="card p-8 rounded-3xl fade-in border-primary/40 bg-gradient-to-b from-primary/10 to-dark-card flex flex-col">
            <p className="text-sm font-semibold text-gray-400 uppercase tracking-wide mb-2">
              {t.pricing_windows_platform}
            </p>
            <div className="flex items-baseline gap-2 mb-1">
              <span className="text-5xl font-bold">{t.pricing_windows_price}</span>
            </div>
            <p className="text-sm text-gray-400 mb-6">{t.pricing_windows_price_note}</p>
            <a
              href={STORE_URL}
              target="_blank"
              rel="noopener noreferrer"
              className="mt-auto inline-block text-center cta-button text-white font-semibold px-8 py-4 rounded-xl transition-all"
            >
              {t.pricing_windows_cta}
            </a>
          </div>

          <div className="card p-8 rounded-3xl fade-in flex flex-col">
            <p className="text-sm font-semibold text-gray-400 uppercase tracking-wide mb-2">
              {t.pricing_mac_platform}
            </p>
            <div className="flex items-baseline gap-2 mb-1">
              <span className="text-5xl font-bold">{t.pricing_mac_price}</span>
            </div>
            <p className="text-sm text-gray-400 mb-6">{t.pricing_mac_price_note}</p>
            <a
              href={RELEASES_URL}
              target="_blank"
              rel="noopener noreferrer"
              className="mt-auto inline-block text-center px-8 py-4 rounded-xl border border-dark-border text-white font-semibold hover:border-primary hover:text-primary transition-colors"
            >
              {t.pricing_mac_cta}
            </a>
          </div>
        </div>

        {/* Capability table — a real table on wide screens, stacked rows on phones */}
        <div className="card rounded-3xl overflow-hidden fade-in">
          <div className="px-6 md:px-8 py-5 border-b border-dark-border">
            <h3 className="text-xl font-bold">{t.pricing_table_title}</h3>
          </div>

          {/* md and up: three columns */}
          <table className="hidden md:table w-full text-left">
            <thead>
              <tr className="text-sm text-gray-400 border-b border-dark-border">
                <th className="px-8 py-3 font-semibold">{t.pricing_col_feature}</th>
                <th className="px-4 py-3 font-semibold text-center">{t.pricing_col_store}</th>
                <th className="px-8 py-3 font-semibold text-center">{t.pricing_col_mac}</th>
              </tr>
            </thead>
            <tbody>
              {rows.map((row, idx) => (
                <tr key={idx} className="border-b border-dark-border/60 last:border-0">
                  <td className="px-8 py-3 text-gray-300">{row.label}</td>
                  <td className="px-4 py-3 text-center">{renderCell(row.store)}</td>
                  <td className="px-8 py-3 text-center">{renderCell(row.mac)}</td>
                </tr>
              ))}
            </tbody>
          </table>

          {/* Phones: each row becomes a label plus the two verdicts */}
          <ul className="md:hidden">
            <li className="grid grid-cols-[1fr_auto_auto] gap-3 px-5 py-2.5 border-b border-dark-border text-xs text-gray-500 font-semibold">
              <span />
              <span className="text-center">{t.pricing_col_store}</span>
              <span className="text-center">{t.pricing_col_mac}</span>
            </li>
            {rows.map((row, idx) => (
              <li
                key={idx}
                className="grid grid-cols-[1fr_auto_auto] gap-3 items-center px-5 py-3 border-b border-dark-border/60 last:border-0"
              >
                <span className="text-sm text-gray-300">{row.label}</span>
                <span className="text-center min-w-[4.5rem]">{renderCell(row.store)}</span>
                <span className="text-center min-w-[4.5rem]">{renderCell(row.mac)}</span>
              </li>
            ))}
          </ul>

          <p className="px-6 md:px-8 py-4 border-t border-dark-border text-xs text-gray-500">
            {t.pricing_footnote}
          </p>
        </div>

        {/* What the money buys, stated plainly */}
        <div className="grid md:grid-cols-2 gap-6 mt-6">
          <div className="card p-6 md:p-8 rounded-3xl fade-in">
            <h3 className="font-bold text-lg mb-2">{t.pricing_note_title}</h3>
            <p className="text-gray-400 leading-relaxed">{t.pricing_note_body}</p>
          </div>
          <div className="card p-6 md:p-8 rounded-3xl fade-in flex items-center">
            <p className="text-gray-400 leading-relaxed">{t.pricing_source_note}</p>
          </div>
        </div>
      </div>
    </section>
  );
};

/** A check is enough for boolean rows; the setup row needs its own wording. */
function renderCell(value: boolean | string) {
  if (value === true) return <span className="text-green-500">✓</span>;
  if (value === false) return <span className="text-gray-600">—</span>;
  return <span className="text-gray-300 text-sm">{value}</span>;
}
