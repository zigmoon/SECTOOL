import { defineConfig } from 'astro/config';

// Application 100 % statique : Astro génère la page, Vite (intégré à Astro)
// empaquette le TypeScript du tableau de bord. Tauri charge le dossier `dist`.
export default defineConfig({
  output: 'static',
  server: { port: 4321, host: '127.0.0.1' },
  devToolbar: { enabled: false },
  build: { inlineStylesheets: 'never' },
  vite: {
    clearScreen: false,
    server: { strictPort: true },
  },
});
