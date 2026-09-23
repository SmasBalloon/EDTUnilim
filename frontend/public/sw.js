/**
 * Service worker minimal.
 *
 * La coquille de l'application est mise en cache à l'installation ; les
 * réponses de l'API sont servies par le réseau, avec repli sur la dernière
 * version connue — l'emploi du temps consulté reste donc lisible hors ligne.
 */

const SHELL = 'edt-shell-v2';
const DATA = 'edt-data-v1';

self.addEventListener('install', (event) => {
  event.waitUntil(
    caches.open(SHELL).then((cache) => cache.addAll(['/', '/manifest.webmanifest', '/icon.svg'])),
  );
  self.skipWaiting();
});

self.addEventListener('activate', (event) => {
  event.waitUntil(
    caches
      .keys()
      .then((keys) =>
        Promise.all(keys.filter((key) => key !== SHELL && key !== DATA).map((key) => caches.delete(key))),
      )
      .then(() => self.clients.claim()),
  );
});

self.addEventListener('fetch', (event) => {
  const request = event.request;
  if (request.method !== 'GET') return;

  const url = new URL(request.url);
  if (url.origin !== self.location.origin) return;

  // Connexion et compte : toujours le réseau, jamais de copie gardée.
  if (url.pathname.startsWith('/api/') && !url.pathname.startsWith('/api/v1/years')) return;
  // L'abonnement iCalendar est lu par les agendas, pas par l'application.
  if (url.pathname.includes('/calendar/')) return;

  // Données : réseau d'abord, cache en secours.
  if (url.pathname.startsWith('/api/')) {
    event.respondWith(
      fetch(request)
        .then((response) => {
          const copy = response.clone();
          void caches.open(DATA).then((cache) => cache.put(request, copy));
          return response;
        })
        .catch(() => caches.match(request).then((hit) => hit ?? Response.error())),
    );
    return;
  }

  // Navigation : la coquille suffit, le reste est reconstruit côté client.
  if (request.mode === 'navigate') {
    event.respondWith(fetch(request).catch(() => caches.match('/').then((hit) => hit ?? Response.error())));
    return;
  }

  // Ressources statiques : cache d'abord, mise à jour en arrière-plan.
  event.respondWith(
    caches.match(request).then(
      (hit) =>
        hit ??
        fetch(request).then((response) => {
          const copy = response.clone();
          void caches.open(SHELL).then((cache) => cache.put(request, copy));
          return response;
        }),
    ),
  );
});
