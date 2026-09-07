import { render } from 'solid-js/web';

import '@fontsource-variable/ibm-plex-sans';
import './app.css';
import { App } from './App';
import { watchTheme } from './lib/settings';

watchTheme();

const root = document.getElementById('root');
if (root) render(() => <App />, root);

// Service worker : coquille de l'application et dernières semaines consultées
// disponibles hors connexion.
if (import.meta.env.PROD && 'serviceWorker' in navigator) {
  window.addEventListener('load', () => {
    void navigator.serviceWorker.register('/sw.js');
  });
}
