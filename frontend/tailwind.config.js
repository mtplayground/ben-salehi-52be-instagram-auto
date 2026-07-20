/** @type {import('tailwindcss').Config} */
export default {
  content: ['./index.html', './src/**/*.{ts,tsx}'],
  theme: {
    extend: {
      colors: {
        ink: '#1f2933',
        meadow: '#2f7d5c',
        coral: '#d95f45',
        sky: '#3a7ca5',
        paper: '#f7f4ed',
      },
      boxShadow: {
        panel: '0 16px 45px rgba(31, 41, 51, 0.08)',
      },
    },
  },
  plugins: [],
};
