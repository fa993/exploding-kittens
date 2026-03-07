import { defineConfig } from 'vite';
import preact from '@preact/preset-vite';

export default defineConfig({
  plugins: [preact()],
  build: {
    outDir: '../dist',
    emptyOutDir: true,
  },
  base: process.env.VITE_APP_BASE_PATH || '/', // build
});