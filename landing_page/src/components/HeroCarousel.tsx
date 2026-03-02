import React, { useState } from 'react';

const images = [
  '/imgs/main-2.png',
  '/imgs/main-3.png',
  '/imgs/monosub-main2.png',
  '/imgs/setting-1.png',
];


export const HeroCarousel: React.FC = () => {
  const [current, setCurrent] = useState(0);
  const next = () => setCurrent((c) => (c + 1) % images.length);
  const prev = () => setCurrent((c) => (c - 1 + images.length) % images.length);

  return (
    <div className="relative w-full h-auto flex flex-col items-center">
      <div className="w-full rounded-2xl overflow-hidden shadow-2xl border border-dark-border">
        <img
          src={images[current]}
          alt={`MonoSub Screenshot ${current + 1}`}
          className="w-full h-auto object-contain"
          loading="eager"
        />
      </div>
      <div className="flex justify-center items-center gap-4 mt-4">
        <button onClick={prev} className="px-3 py-1 rounded bg-gray-700 text-white">&#8592;</button>
        {images.map((_, idx) => (
          <span
            key={idx}
            className={`inline-block w-3 h-3 rounded-full mx-1 ${idx === current ? 'bg-primary' : 'bg-gray-400'}`}
            onClick={() => setCurrent(idx)}
            style={{ cursor: 'pointer' }}
          />
        ))}
        <button onClick={next} className="px-3 py-1 rounded bg-gray-700 text-white">&#8594;</button>
      </div>
    </div>
  );
};
