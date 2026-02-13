/** @type {import('tailwindcss').Config} */

export default {
  darkMode: "class",
  content: ["./index.html", "./src/**/*.{js,ts,jsx,tsx}"],
  theme: {
    container: {
      center: true,
    },
    extend: {
      colors: {
        // Modern Apple-inspired color palette
        primary: {
          DEFAULT: '#34C759',  // Apple Green
          hover: '#2EB24F',
          light: '#E8F8EC',
        },
        accent: {
          DEFAULT: '#5AC8FA',  // Apple Teal/Blue accent
          hover: '#3EBAF8',
          light: '#E8F7FE',
        },
        teal: {
          DEFAULT: '#5AC8FA',  // Apple Teal
          hover: '#3EBAF8',
          light: '#E8F7FE',
        },
        bg: {
          page: '#FFFFFF',
          card: '#FFFFFF',
          secondary: '#F5F5F7',  // Apple Gray Background
          tertiary: '#FAFAFA',
        },
        text: {
          primary: '#1D1D1F',   // Apple Black
          secondary: '#86868B', // Apple Gray
          tertiary: '#AEAEB2',
        },
        border: '#D2D2D7',      // Apple Border
        divider: '#E8E8ED',
        success: '#34C759',     // Apple Green
        warning: '#FF9500',     // Apple Orange
        danger: '#FF3B30',      // Apple Red
        info: '#007AFF',        // Apple Blue (for info/links)

        // Dark mode glass backgrounds (2026 Liquid Glass aesthetic)
        'dark-glass': {
          50: 'rgba(30, 30, 40, 0.4)',
          100: 'rgba(30, 30, 40, 0.5)',
          200: 'rgba(30, 30, 40, 0.6)',
          300: 'rgba(30, 30, 40, 0.7)',
        },

        // Glass borders for glassmorphism
        'glass-border-light': 'rgba(255, 255, 255, 0.12)',
        'glass-border-dark': 'rgba(255, 255, 255, 0.08)',
      },
      borderRadius: {
        sm: '6px',
        md: '10px',
        lg: '14px',
        xl: '18px',
        '2xl': '20px',  // macOS window corners
        '3xl': '24px',  // larger cards
      },
      boxShadow: {
        'apple': '0 2px 8px rgba(0, 0, 0, 0.08)',
        'apple-lg': '0 4px 16px rgba(0, 0, 0, 0.12)',
        'apple-inset': 'inset 0 1px 0 rgba(255, 255, 255, 0.1)',
        // Glass shadows for Liquid Glass aesthetic
        'glass-sm': '0 2px 12px rgba(0, 0, 0, 0.05)',
        'glass-md': '0 4px 20px rgba(0, 0, 0, 0.08)',
        'glass-lg': '0 8px 32px rgba(0, 0, 0, 0.12)',
        'float': '0 8px 24px rgba(52, 199, 89, 0.2)',
        'float-hover': '0 12px 32px rgba(52, 199, 89, 0.26)',
      },
      fontFamily: {
        sans: [
          '-apple-system',
          'BlinkMacSystemFont',
          'SF Pro Display',
          'SF Pro Text',
          'Helvetica Neue',
          'Arial',
          'sans-serif',
        ],
      },
      fontSize: {
        'xxs': ['0.6875rem', { lineHeight: '1rem' }],       // 11px
        'xs': ['0.75rem', { lineHeight: '1.125rem' }],     // 12px
        'sm': ['0.8125rem', { lineHeight: '1.25rem' }],    // 13px
        'base': ['0.9375rem', { lineHeight: '1.5rem' }],   // 15px
        'lg': ['1.0625rem', { lineHeight: '1.625rem' }],   // 17px
        'xl': ['1.25rem', { lineHeight: '1.75rem' }],      // 20px
        '2xl': ['1.5rem', { lineHeight: '2rem' }],         // 24px
      },
      animation: {
        'fade-in': 'fadeIn 0.2s ease-out',
        'slide-up': 'slideUp 0.3s ease-out',
        'pulse-soft': 'pulseSoft 2s ease-in-out infinite',
      },
      keyframes: {
        fadeIn: {
          '0%': { opacity: '0' },
          '100%': { opacity: '1' },
        },
        slideUp: {
          '0%': { opacity: '0', transform: 'translateY(10px)' },
          '100%': { opacity: '1', transform: 'translateY(0)' },
        },
        pulseSoft: {
          '0%, 100%': { opacity: '1' },
          '50%': { opacity: '0.7' },
        },
      },
      transitionDuration: {
        '250': '250ms',
      },
    },
  },
  plugins: [require("tailwindcss-animate")],
};
