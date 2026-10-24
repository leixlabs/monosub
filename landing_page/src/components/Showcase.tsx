import { useRef, useState } from 'react';
import { useLanguage } from '../hooks/useLanguage';
import { shot, shotSize, type ShotName } from '../lib/screenshots';

type TabId = 'queue' | 'translate' | 'speakers' | 'url';

/**
 * The product tour: real screenshots of the running app, grouped by what the
 * visitor is trying to find out. Every image is a capture of the app itself —
 * nothing here is a mock-up.
 */
const TOURS: {
  id: TabId;
  shots: { name: ShotName; caption: string }[];
}[] = [
  {
    id: 'queue',
    shots: [
      { name: 'queue', caption: 'shot_queue' },
      { name: 'batchRunning', caption: 'shot_batch' },
      { name: 'rowStates', caption: 'shot_rowstates' },
    ],
  },
  {
    id: 'translate',
    shots: [
      { name: 'translated', caption: 'shot_translated' },
      { name: 'translationSettings', caption: 'shot_translate_settings' },
    ],
  },
  {
    id: 'speakers',
    shots: [
      { name: 'models', caption: 'shot_models' },
      { name: 'modelCatalogue', caption: 'shot_catalogue' },
    ],
  },
  {
    id: 'url',
    shots: [
      { name: 'urlImportRows', caption: 'shot_url_rows' },
      { name: 'urlDialog', caption: 'shot_url_dialog' },
      { name: 'clipboardPill', caption: 'shot_clipboard' },
    ],
  },
];

export const Showcase = () => {
  const { t, lang } = useLanguage();
  const [tab, setTab] = useState(0);
  const [shotIndex, setShotIndex] = useState(0);
  const tabRefs = useRef<(HTMLButtonElement | null)[]>([]);

  const tour = TOURS[tab];
  // A tab with fewer shots than the one before it must not leave the selection
  // pointing past its end.
  const activeIndex = Math.min(shotIndex, tour.shots.length - 1);
  const active = tour.shots[activeIndex];
  const activeSize = shotSize(active.name);

  const selectTab = (idx: number) => {
    setTab(idx);
    setShotIndex(0);
  };

  // Arrow keys move between tabs, as the tablist pattern requires.
  const onTabKeyDown = (event: React.KeyboardEvent, idx: number) => {
    const last = TOURS.length - 1;
    let next: number | null = null;
    if (event.key === 'ArrowRight') next = idx === last ? 0 : idx + 1;
    if (event.key === 'ArrowLeft') next = idx === 0 ? last : idx - 1;
    if (event.key === 'Home') next = 0;
    if (event.key === 'End') next = last;
    if (next === null) return;

    event.preventDefault();
    selectTab(next);
    tabRefs.current[next]?.focus();
  };

  return (
    <section id="screenshots" className="py-16 px-6 scroll-mt-24">
      <div className="max-w-7xl mx-auto">
        <div className="text-center mb-10 fade-in">
          <h2 className="text-4xl md:text-5xl font-bold mb-4">{t.showcase_title}</h2>
          <p className="text-xl text-gray-400 max-w-2xl mx-auto">{t.showcase_subtitle}</p>
        </div>

        {/* Tabs — one scrollable row on a phone, centred wrap on a wide screen */}
        <div className="mb-8 -mx-6 px-6 overflow-x-auto md:overflow-visible [scrollbar-width:none] [&::-webkit-scrollbar]:hidden">
          <div
            className="flex items-center gap-2 md:gap-3 w-max md:w-auto md:justify-center md:flex-wrap mx-auto"
            role="tablist"
            aria-label={t.showcase_title}
          >
            {TOURS.map((item, idx) => {
              const selected = idx === tab;
              return (
                <button
                  key={item.id}
                  ref={(el) => {
                    tabRefs.current[idx] = el;
                  }}
                  type="button"
                  role="tab"
                  id={`showcase-tab-${item.id}`}
                  aria-selected={selected}
                  aria-controls="showcase-panel"
                  tabIndex={selected ? 0 : -1}
                  onClick={() => selectTab(idx)}
                  onKeyDown={(event) => onTabKeyDown(event, idx)}
                  className={`shrink-0 px-5 py-2.5 rounded-full border text-sm font-semibold transition-colors ${
                    selected
                      ? 'bg-primary/10 border-primary text-primary'
                      : 'bg-dark-card border-dark-border text-gray-400 hover:text-white hover:border-gray-500'
                  }`}
                >
                  {t[`showcase_tab_${item.id}` as const]}
                </button>
              );
            })}
          </div>
        </div>

        <div
          id="showcase-panel"
          role="tabpanel"
          aria-labelledby={`showcase-tab-${tour.id}`}
          className="grid grid-cols-1 lg:grid-cols-3 gap-6 items-start"
        >
          {/* The selected screenshot — the largest thing in the section */}
          <div className="lg:col-span-2 card p-3 md:p-4 rounded-3xl border-primary/40 bg-gradient-to-b from-dark-card to-dark-bg fade-in">
            <div className="rounded-2xl overflow-hidden border border-dark-border bg-black">
              <img
                key={active.name}
                src={shot(active.name, lang)}
                alt={t[active.caption as keyof typeof t]}
                width={activeSize.width}
                height={activeSize.height}
                className="w-full h-auto block"
                loading="lazy"
                decoding="async"
              />
            </div>
            <p className="px-2 pt-3 pb-1 text-sm text-gray-400 text-center">
              {t[active.caption as keyof typeof t]}
            </p>
          </div>

          {/* What this tab is about, plus the other shots in it */}
          <div className="card p-6 md:p-7 rounded-3xl fade-in">
            <h3 className="text-xl font-bold mb-3">{t[`showcase_tab_${tour.id}` as const]}</h3>
            <p className="text-gray-400 leading-relaxed mb-6">
              {t[`showcase_${tour.id}_desc` as const]}
            </p>

            {tour.shots.length > 1 && (
              <div className="grid grid-cols-2 gap-3">
                {tour.shots.map((item, idx) => (
                  <button
                    key={item.name}
                    type="button"
                    onClick={() => setShotIndex(idx)}
                    aria-label={t[item.caption as keyof typeof t]}
                    aria-pressed={idx === activeIndex}
                    className={`rounded-xl overflow-hidden border transition-colors ${
                      idx === activeIndex
                        ? 'border-primary'
                        : 'border-dark-border hover:border-gray-500'
                    }`}
                  >
                    <img
                      src={shot(item.name, lang)}
                      alt={t[item.caption as keyof typeof t]}
                      width={shotSize(item.name).width}
                      height={shotSize(item.name).height}
                      className="w-full h-auto block"
                      loading="lazy"
                      decoding="async"
                    />
                  </button>
                ))}
              </div>
            )}
          </div>
        </div>
      </div>
    </section>
  );
};
