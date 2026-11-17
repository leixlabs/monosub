import { useRef, useState } from 'react';
import { useLanguage } from '../hooks/useLanguage';
import { shot, shotSize, type ShotName } from '../lib/screenshots';

/** Shot names per tour, index-aligned with the captions in `t.showcase.tours`. */
const TOUR_SHOTS: Record<string, ShotName[]> = {
  queue: ['queue', 'batchRunning', 'rowStates'],
  translate: ['translated', 'translationSettings'],
  speakers: ['models', 'modelCatalogue'],
  url: ['urlImportRows', 'urlDialog', 'clipboardPill'],
};

export const Showcase = () => {
  const { t, lang } = useLanguage();
  const [tab, setTab] = useState(0);
  const [shotIndex, setShotIndex] = useState(0);
  const tabRefs = useRef<(HTMLButtonElement | null)[]>([]);

  const tours = t.showcase.tours;
  const tour = tours[tab];
  const names = TOUR_SHOTS[tour.id];
  const activeIndex = Math.min(shotIndex, names.length - 1);
  const activeName = names[activeIndex];
  const activeSize = shotSize(activeName);

  const selectTab = (idx: number) => {
    setTab(idx);
    setShotIndex(0);
  };

  const onTabKeyDown = (event: React.KeyboardEvent, idx: number) => {
    const last = tours.length - 1;
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
    <section id="screenshots" className="section scroll-mt-24">
      <div className="container-page">
        <div className="flex flex-col md:flex-row md:items-end justify-between gap-6">
          <div className="section-head">
            <p className="eyebrow">{t.showcase.eyebrow}</p>
            <h2 className="display-2 mt-5">{t.showcase.title}</h2>
          </div>
          <p className="text-ink-6 max-w-[34ch] md:text-right">{t.showcase.subtitle}</p>
        </div>

        {/* Tabs */}
        <div className="mt-9 -mx-1 px-1 overflow-x-auto [scrollbar-width:none] [&::-webkit-scrollbar]:hidden">
          <div className="flex items-center gap-6 md:gap-8 w-max border-b border-line" role="tablist" aria-label={t.showcase.title}>
            {tours.map((item, idx) => {
              const selected = idx === tab;
              return (
                <button
                  key={item.id}
                  ref={(el) => { tabRefs.current[idx] = el; }}
                  type="button"
                  role="tab"
                  id={`tour-tab-${item.id}`}
                  aria-selected={selected}
                  aria-controls="tour-panel"
                  tabIndex={selected ? 0 : -1}
                  onClick={() => selectTab(idx)}
                  onKeyDown={(event) => onTabKeyDown(event, idx)}
                  className={`shrink-0 -mb-px bg-transparent border-0 border-b-2 pb-3 pt-1 text-sm font-semibold transition-colors ${
                    selected
                      ? 'border-gold text-ink'
                      : 'border-transparent text-ink-6 hover:text-ink'
                  }`}
                >
                  {item.label}
                </button>
              );
            })}
          </div>
        </div>

        <div
          id="tour-panel"
          role="tabpanel"
          aria-labelledby={`tour-tab-${tour.id}`}
          className="mt-8 grid lg:grid-cols-12 gap-8 items-start"
        >
          <div className="lg:col-span-8">
            <div className="shot-frame">
              <img
                key={activeName}
                src={shot(activeName, lang)}
                alt={tour.shots[activeIndex]}
                width={activeSize.width}
                height={activeSize.height}
                loading="lazy"
                decoding="async"
              />
            </div>
            <p className="mt-3 text-xs text-ink-6 text-center">{tour.shots[activeIndex]}</p>
          </div>

          <div className="lg:col-span-4">
            <h3 className="font-display text-xl">{tour.label}</h3>
            <p className="mt-3 text-ink-7 leading-relaxed">{tour.desc}</p>

            {tour.shots.length > 1 && (
              <div className="mt-6 grid grid-cols-2 gap-3">
                {names.map((name, idx) => (
                  <button
                    key={name}
                    type="button"
                    onClick={() => setShotIndex(idx)}
                    aria-label={tour.shots[idx]}
                    aria-pressed={idx === activeIndex}
                    className={`rounded overflow-hidden border bg-paper p-0.5 transition-colors ${
                      idx === activeIndex ? 'border-gold' : 'border-line hover:border-ink-5'
                    }`}
                  >
                    <img
                      src={shot(name, lang)}
                      alt={tour.shots[idx]}
                      width={shotSize(name).width}
                      height={shotSize(name).height}
                      loading="lazy"
                      decoding="async"
                      className="block w-full h-auto rounded-sm"
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
