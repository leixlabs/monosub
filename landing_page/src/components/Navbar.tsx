import { useEffect, useState } from 'react';
import { Link, useLocation } from 'react-router-dom';
import { useLanguage } from '../hooks/useLanguage';
import { LanguageSwitcher } from './LanguageSwitcher';
import { APP_STORE_URL } from '../lib/links';
import { BLOG_PATH, pagePath } from '../lib/seo';

/**
 * One navigation at every size. Wide: inline links in the header. Narrow: the
 * same links inside a native `popover`, opened by a menu button — no custom
 * focus trap, Escape and light-dismiss come from the platform.
 */
export const Navbar = () => {
  const { t, lang } = useLanguage();
  const { pathname } = useLocation();
  const [scrolled, setScrolled] = useState(false);

  useEffect(() => {
    const onScroll = () => setScrolled(window.scrollY > 8);
    onScroll();
    window.addEventListener('scroll', onScroll, { passive: true });
    return () => window.removeEventListener('scroll', onScroll);
  }, []);

  const home = pagePath(lang);
  const isHome = pathname === home || pathname === `${home}/`;
  const sectionLink = (hash: string) => (isHome ? `#${hash}` : `${home}#${hash}`);
  const blogHref = BLOG_PATH[lang === 'zh' ? 'zh' : 'en'];

  const links = [
    { href: sectionLink('features'), label: t.nav.features },
    { href: sectionLink('how'), label: t.nav.how },
    { href: sectionLink('pricing'), label: t.nav.pricing },
    { href: sectionLink('faq'), label: t.nav.faq },
  ];

  return (
    <header
      className={`sticky top-0 z-50 border-b transition-colors duration-200 ${
        scrolled ? 'bg-paper/90 backdrop-blur-md border-line' : 'bg-paper/0 border-transparent'
      }`}
    >
      <div className="container-page flex items-center justify-between gap-4 min-h-[64px]">
        <Link
          to={home}
          className="font-display text-[1.35rem] font-semibold tracking-tight text-ink no-underline"
        >
          MonoSub<span className="text-gold">.</span>
        </Link>

        {/* Wide */}
        <nav className="hidden lg:flex items-center gap-0.5" aria-label="Primary">
          {links.map((link) => (
            <a
              key={link.href}
              href={link.href}
              className="px-3 py-2 text-sm text-ink-7 no-underline rounded hover:text-gold transition-colors"
            >
              {link.label}
            </a>
          ))}
          <Link
            to={blogHref}
            className={`px-3 py-2 text-sm no-underline rounded transition-colors ${
              pathname.includes('/blog') ? 'text-gold' : 'text-ink-7 hover:text-gold'
            }`}
          >
            {t.nav.blog}
          </Link>
        </nav>

        <div className="hidden lg:flex items-center gap-3">
          <LanguageSwitcher />
          <a
            href={APP_STORE_URL}
            target="_blank"
            rel="noopener noreferrer"
            className="btn btn-primary text-sm"
          >
            {t.nav.cta_primary}
          </a>
        </div>

        {/* Narrow */}
        <div className="lg:hidden flex items-center gap-2">
          <a
            href={APP_STORE_URL}
            target="_blank"
            rel="noopener noreferrer"
            className="btn btn-primary text-sm min-h-[2.4rem] px-3 py-2"
          >
            {t.nav.cta_primary}
          </a>
          <button
            type="button"
            {...({ popovertarget: 'mobile-nav', popovertargetaction: 'toggle' } as unknown as React.ButtonHTMLAttributes<HTMLButtonElement>)}
            aria-label="Menu"
            className="grid place-items-center w-10 h-10 rounded border border-line text-ink bg-paper"
          >
            <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" aria-hidden="true">
              <path d="M3 6h18M3 12h18M3 18h18" />
            </svg>
          </button>

          <nav id="mobile-nav" {...({ popover: 'auto' } as unknown as React.HTMLAttributes<HTMLElement>)} className="mobile-nav" aria-label="Primary">
            <ul>
              {links.map((link) => (
                <li key={link.href}>
                  <a href={link.href} onClick={() => (document.getElementById('mobile-nav') as HTMLElement & { hidePopover?: () => void })?.hidePopover?.()}>
                    {link.label}
                  </a>
                </li>
              ))}
              <li>
                <Link to={blogHref} onClick={() => (document.getElementById('mobile-nav') as HTMLElement & { hidePopover?: () => void })?.hidePopover?.()}>
                  {t.nav.blog}
                </Link>
              </li>
            </ul>
            <div className="mobile-nav__foot">
              <a
                href={APP_STORE_URL}
                target="_blank"
                rel="noopener noreferrer"
                className="btn btn-primary btn-block"
              >
                {t.nav.cta_primary}
              </a>
            </div>
          </nav>
        </div>
      </div>
    </header>
  );
};
