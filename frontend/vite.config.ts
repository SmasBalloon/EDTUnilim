import { defineConfig } from 'vite';
import solid from 'vite-plugin-solid';
import tailwind from '@tailwindcss/vite';

/**
 * En développement, `/api` est proxyfié vers l'API Rust : le front tape donc
 * toujours des URL relatives, en dev comme en production.
 */
export default defineConfig({
  plugins: [solid(), tailwind()],
  server: {
    port: 5173,
    proxy: {
      '/api': {
        target: process.env.EDT_API_TARGET ?? 'http://localhost:3000',
        changeOrigin: true,
      },
    },
  },
  build: { target: 'es2022' },
});
