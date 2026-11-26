// The build supplies a revision and an exact allowlist of public shell assets.
const cacheName = 'flo-shell-' + SHELL_VERSION;
const assets = new Set(SHELL_ASSETS);

self.addEventListener('install', (event) => {
  event.waitUntil(caches.open(cacheName).then((cache) => cache.addAll(
    [...assets].map((path) => new Request(path, { cache: 'reload' }))
  )).then(() => self.skipWaiting()));
});

self.addEventListener('activate', (event) => {
  event.waitUntil(caches.keys().then((names) => Promise.all(
    names.filter((name) => name.startsWith('flo-shell-') && name !== cacheName).map((name) => caches.delete(name))
  )).then(() => self.clients.claim()));
});

self.addEventListener('fetch', (event) => {
  const request = event.request;
  const url = new URL(request.url);
  if (request.method !== 'GET' || url.origin !== self.location.origin) return;

  if (request.mode === 'navigate' && url.pathname === '/') {
    event.respondWith(fetch(request).then((response) => {
      if (response.status < 500) return response;
      return caches.open(cacheName).then((cache) => cache.match('/')).then((shell) => shell ?? response);
    }, () => caches.open(cacheName).then((cache) => cache.match('/')).then((shell) => shell ?? Response.error())));
  } else if (assets.has(url.pathname)) {
    event.respondWith(caches.open(cacheName).then((cache) => cache.match(url.pathname)).then((asset) => asset ?? fetch(request)));
  }
});
