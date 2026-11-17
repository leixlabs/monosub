/** @type {import('tailwindcss').Config} */
export default {
  content: ['./index.html', './src/**/*.{js,ts,jsx,tsx}'],
  theme: {
    extend: {
      colors: {
        // ── lei-brand · 方案 A 金墨宣 ─────────────────────────────
        // 只用三个注册色派生的暖中性 + 单一金。第二色相一律不进。
        paper: {
          DEFAULT: '#F7F2E9', // 宣纸白（页面底）
          2: '#EDE5D6',
        },
        ink: {
          DEFAULT: '#13100D', // 墨 9：标题 / 深底
          8: '#2D2925', // 正文
          7: '#4D4843',
          6: '#6F6963', // 次要文字
          5: '#938D86',
          3: '#D5CEC7', // 分隔线
          2: '#E8E2DB',
        },
        gold: {
          DEFAULT: '#965900', // 金 700 注册色
          800: '#774600',
          700: '#965900',
          600: '#B77600',
          500: '#CC9140', // 深底强调
          400: '#E1B175',
          300: '#EFCEA6', // 高亮底
          200: '#F9E4CB',
          100: '#FEF3E7',
        },
        success: '#25855B',
        danger: '#8D1C11',
        line: '#D5CEC7',
      },
      fontFamily: {
        // 系统栈优先：不依赖外部字体，避免国内网络与 CWV 问题。
        display: [
          'ui-serif',
          '"Source Serif 4"',
          '"Songti SC"',
          '"STSong"',
          '"Noto Serif CJK SC"',
          '"SimSun"',
          'Georgia',
          'serif',
        ],
        sans: [
          '-apple-system',
          'BlinkMacSystemFont',
          '"Segoe UI"',
          '"PingFang SC"',
          '"Hiragino Sans GB"',
          '"Microsoft YaHei"',
          'Inter',
          'sans-serif',
        ],
        mono: ['ui-monospace', 'SFMono-Regular', '"JetBrains Mono"', 'Menlo', 'monospace'],
      },
      maxWidth: {
        page: '1120px',
        prose: '68ch',
      },
      transitionTimingFunction: {
        brand: 'cubic-bezier(0.22, 0.61, 0.36, 1)',
      },
    },
  },
  plugins: [],
};
