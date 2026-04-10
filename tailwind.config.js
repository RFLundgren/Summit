/** @type {import('tailwindcss').Config} */
export default {
  content: ['./index.html', './src/**/*.{js,ts,jsx,tsx}'],
  theme: {
    extend: {
      colors: {
        immich: {
          primary: '#4250AF',
          hover: '#3a47a0',
          light: '#eef0fa',
        },
      },
    },
  },
  plugins: [],
}
