import { useCallback, useEffect, useState } from 'react';
import { useLanguage } from '../hooks/useLanguage';
import { shot, shotSize, type ShotName } from '../lib/screenshots';

const SLIDE_MS = 7000;

const SLIDES: { name: ShotName; caption: string }[] = [
  { name: 'queue', caption: 'hero_shot_1' },
  { name: 'batchRunning', caption: 'hero_shot_2' },
  { name: 'translated', caption: 'hero_shot_3' },
  { name: 'clipboardPill', caption: 'hero_shot_4' },
  { name: 'lightTheme', caption: 'hero_shot_5' },
];

export const HeroCarousel = () => {
  const { t, lang } = useLanguage();
  const [current, setCurrent] = useState(0);
  const [paused, setPaused] = useState(false);

  const go = useCallback((next: number) => {
    setCurrent(((next % SLIDES.length) + SLIDES.length) % SLIDES.length);
  }, []);

  // Auto-advance, but only while the pointer is elsewhere: a slide the visitor is
  // reading must not move under them. Any manual interaction also pauses it.
  useEffect(() => {
    if (paused) return;
    if (window.matchMedia('(prefers-reduced-motion: reduce)').matches) return;

    const id = window.setTimeout(() => go(current + 1), SLIDE_MS);
    return () => window.clearTimeout(id);
  }, [current, paused, go]);

  const slide = SLIDES[current];
  const size = shotSize(slide.name);

  return (
    <div
      className="relative w-full flex flex-col"
      onMouseEnter={() => setPaused(true)}
      onMouseLeave={() => setPaused(false)}
      onFocus={() => setPaused(true)}
      onBlur={() => setPaused(false)}
    >
      <div className="relative w-full rounded-2xl overflow-hidden border border-dark-border bg-black shadow-2xl">
        <img
          key={slide.name}
          src={shot(slide.name, lang)}
          alt={t[slide.caption as keyof typeof t] as string}
          width={size.width}
          height={size.height}
          className="w-full h-auto block"
          loading="eager"
          decoding="async"
          fetchPriority={current === 0 ? 'high' : 'auto'}
        />
      </div>

      <div className="flex items-center justify-between gap-3 mt-4">
        <button
          type="button"
          onClick={() => go(current - 1)}
          aria-label={t.carousel_prev}
          className="w-9 h-9 shrink-0 rounded-full border border-dark-border text-gray-300 hover:text-white hover:border-primary transition-colors flex items-center justify-center"
        >
          <svg className="w-4 h-4" fill="none" stroke="currentColor" strokeWidth={2} viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" d="M15 19l-7-7 7-7" />
          </svg>
        </button>

        <p className="flex-1 text-center text-sm text-gray-400 truncate" aria-live="polite">
          {t[slide.caption as keyof typeof t] as string}
        </p>

        <div className="flex items-center gap-1.5 shrink-0">
          {SLIDES.map((item, idx) => (
            <button
              key={item.name}
              type="button"
              onClick={() => go(idx)}
              aria-label={t[item.caption as keyof typeof t] as string}
              aria-current={idx === current}
              className={`h-1.5 rounded-full transition-all ${
                idx === current ? 'w-6 bg-primary' : 'w-1.5 bg-dark-border hover:bg-gray-500'
              }`}
            />
          ))}
        </div>

        <button
          type="button"
          onClick={() => go(current + 1)}
          aria-label={t.carousel_next}
          className="w-9 h-9 shrink-0 rounded-full border border-dark-border text-gray-300 hover:text-white hover:border-primary transition-colors flex items-center justify-center"
        >
          <svg className="w-4 h-4" fill="none" stroke="currentColor" strokeWidth={2} viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" d="M9 5l7 7-7 7" />
          </svg>
        </button>
      </div>
    </div>
  );
};
