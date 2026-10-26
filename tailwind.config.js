/** @type {import('tailwindcss').Config} */

/**
 * MonoSub design tokens.
 *
 * Brand colours are CSS variables (see `src/index.css`) in `R G B` channel form so
 * Tailwind opacity modifiers (`bg-primary/10`) keep working AND the light/dark pair
 * lives in one place. Token *names* are unchanged from the previous palette, so
 * every existing `bg-bg-secondary` / `text-text-tertiary` / `text-primary` still
 * resolves — only the values were corrected.
 */
const withAlpha = (name) => `rgb(var(${name}) / <alpha-value>)`;

export default {
  darkMode: "class",
  content: ["./index.html", "./src/**/*.{js,ts,jsx,tsx}"],
  theme: {
    container: {
      center: true,
    },
    extend: {
      colors: {
        // One brand colour. Deep green in light mode (white text passes 4.5:1),
        // brighter green in dark mode (dark text passes). Replaces the old
        // green + cyan→blue→purple gradient mix.
        primary: {
          DEFAULT: withAlpha('--primary'),
          hover: withAlpha('--primary-hover'),
          light: withAlpha('--primary-soft'),
          foreground: withAlpha('--primary-foreground'),
        },
        // Secondary accent, used sparingly (links, info highlights).
        accent: {
          DEFAULT: withAlpha('--accent'),
          hover: withAlpha('--accent-hover'),
          light: withAlpha('--accent-soft'),
        },
        teal: {
          DEFAULT: withAlpha('--accent'),
          hover: withAlpha('--accent-hover'),
          light: withAlpha('--accent-soft'),
        },
        // Neutral surfaces (light values; dark pairs are applied with `dark:`).
        bg: {
          page: '#FFFFFF',
          card: '#FFFFFF',
          secondary: '#F5F6F8', // app background
          tertiary: '#EEF0F3',  // hover / inset fills
        },
        // Semantic surfaces that switch automatically — prefer these in new code.
        surface: {
          DEFAULT: withAlpha('--surface'),
          muted: withAlpha('--surface-muted'),
          raised: withAlpha('--surface-raised'),
        },
        line: {
          DEFAULT: 'rgb(var(--line) / var(--line-alpha))',
          strong: 'rgb(var(--line) / var(--line-strong-alpha))',
        },
        text: {
          primary: '#16181D',
          secondary: '#5B616B', // 5.9:1 on white (was #86868B, 3.5:1)
          tertiary: '#80858F',  // meta / captions
        },
        border: '#DADDE3',
        divider: '#E6E8EC',
        success: '#1F9D55',
        warning: '#D97706',
        danger: '#DC3545',
        info: '#2563EB',

        // Kept for backwards compatibility with existing class names.
        'dark-glass': {
          50: 'rgba(24, 24, 27, 0.4)',
          100: 'rgba(24, 24, 27, 0.5)',
          200: 'rgba(24, 24, 27, 0.6)',
          300: 'rgba(24, 24, 27, 0.7)',
        },
        'glass-border-light': 'rgba(255, 255, 255, 0.12)',
        'glass-border-dark': 'rgba(255, 255, 255, 0.08)',
      },
      // Modern radius scale: controls 8–10, cards 12–16, dialogs 20.
      borderRadius: {
        sm: '6px',
        md: '8px',
        lg: '10px',
        xl: '12px',
        '2xl': '16px',
        '3xl': '20px',
      },
      // Soft, layered, low-opacity shadows. No heavy black drops.
      boxShadow: {
        'card': '0 1px 2px rgba(16, 24, 40, 0.04), 0 1px 3px rgba(16, 24, 40, 0.06)',
        'card-hover': '0 4px 12px rgba(16, 24, 40, 0.08), 0 1px 3px rgba(16, 24, 40, 0.06)',
        'pop': '0 12px 32px -4px rgba(16, 24, 40, 0.18), 0 2px 8px rgba(16, 24, 40, 0.08)',
        'ring-primary': '0 0 0 3px rgb(var(--primary) / 0.22)',
        // Legacy names (still referenced by some components), softened.
        'apple': '0 1px 2px rgba(16, 24, 40, 0.06)',
        'apple-lg': '0 4px 12px rgba(16, 24, 40, 0.1)',
        'apple-inset': 'inset 0 1px 0 rgba(255, 255, 255, 0.1)',
        'glass-sm': '0 1px 3px rgba(16, 24, 40, 0.06)',
        'glass-md': '0 2px 8px rgba(16, 24, 40, 0.08)',
        'glass-lg': '0 12px 32px -4px rgba(16, 24, 40, 0.18), 0 2px 8px rgba(16, 24, 40, 0.08)',
        'float': '0 4px 14px rgb(var(--primary) / 0.28)',
        'float-hover': '0 6px 18px rgb(var(--primary) / 0.36)',
      },
      fontFamily: {
        // Windows + macOS + CJK fallbacks. The old stack fell through to Arial on
        // Windows, which made Chinese text render in a mismatched system font.
        sans: [
          'Inter',
          '-apple-system',
          'BlinkMacSystemFont',
          'SF Pro Text',
          'Segoe UI Variable Text',
          'Segoe UI',
          'PingFang SC',
          'Microsoft YaHei UI',
          'Helvetica Neue',
          'Arial',
          'sans-serif',
        ],
      },
      // Type scale: Caption 11 / Small 12 / Body-sm 13 / Body 14 / H3 16 / H2 20 / H1 24.
      fontSize: {
        'xxs': ['0.6875rem', { lineHeight: '1rem', letterSpacing: '0.01em' }],      // 11 caption
        'xs': ['0.75rem', { lineHeight: '1.125rem', letterSpacing: '0.005em' }],     // 12 meta
        'sm': ['0.8125rem', { lineHeight: '1.25rem' }],                              // 13 secondary body
        'base': ['0.875rem', { lineHeight: '1.375rem' }],                            // 14 body
        'lg': ['1rem', { lineHeight: '1.5rem', letterSpacing: '-0.005em' }],         // 16 H3
        'xl': ['1.25rem', { lineHeight: '1.75rem', letterSpacing: '-0.01em' }],      // 20 H2
        '2xl': ['1.5rem', { lineHeight: '2rem', letterSpacing: '-0.015em' }],        // 24 H1
      },
      transitionTimingFunction: {
        'out-expo': 'cubic-bezier(0.16, 1, 0.3, 1)',
        'standard': 'cubic-bezier(0.4, 0, 0.2, 1)',
      },
      animation: {
        'fade-in': 'fadeIn 0.2s cubic-bezier(0.4, 0, 0.2, 1)',
        'slide-up': 'slideUp 0.3s cubic-bezier(0.16, 1, 0.3, 1)',
        'pulse-soft': 'pulseSoft 2s ease-in-out infinite',
        'shimmer': 'shimmer 1.6s linear infinite',
      },
      keyframes: {
        fadeIn: {
          '0%': { opacity: '0' },
          '100%': { opacity: '1' },
        },
        slideUp: {
          '0%': { opacity: '0', transform: 'translateY(8px)' },
          '100%': { opacity: '1', transform: 'translateY(0)' },
        },
        pulseSoft: {
          '0%, 100%': { opacity: '1' },
          '50%': { opacity: '0.7' },
        },
        shimmer: {
          '0%': { backgroundPosition: '200% 0' },
          '100%': { backgroundPosition: '-200% 0' },
        },
      },
      transitionDuration: {
        '250': '250ms',
      },
    },
  },
  plugins: [require("tailwindcss-animate")],
};
