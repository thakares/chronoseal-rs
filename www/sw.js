/* ============================================================
   ChronoSeal Website — Service Worker (sw.js)
   Enables PWA offline capabilities and asset caching
   ============================================================ */

var CACHE_NAME = 'chronoseal-cache-v1';
var ASSETS = [
  '/',
  '/index.html',
  '/architecture.html',
  '/protocol.html',
  '/api.html',
  '/deployment.html',
  '/threat-model.html',
  '/performance.html',
  '/philosophy.html',
  '/privacy.html',
  '/security.html',
  '/operations.html',
  '/testing.html',
  '/comparison.html',
  '/css/chronoseal.css',
  '/css/docs.css',
  '/css/print.css',
  '/js/app.js',
  '/js/search.js',
  '/js/diagrams.js',
  '/js/pwa.js',
  '/assets/logo.svg',
  '/assets/logo.png',
  '/site.webmanifest',
  '/robots.txt'
];

/* Install Event — Pre-cache critical assets */
self.addEventListener('install', function (e) {
  e.waitUntil(
    caches.open(CACHE_NAME).then(function (cache) {
      return cache.addAll(ASSETS);
    }).then(function () {
      return self.skipWaiting();
    })
  );
});

/* Activate Event — Clean up old caches */
self.addEventListener('activate', function (e) {
  e.waitUntil(
    caches.keys().then(function (keys) {
      return Promise.all(
        keys.map(function (key) {
          if (key !== CACHE_NAME) {
            return caches.delete(key);
          }
        })
      );
    }).then(function () {
      return self.clients.claim();
    })
  );
});

/* Fetch Event — Cache-first with Network Fallback */
self.addEventListener('fetch', function (e) {
  // Only handle GET requests and local requests
  if (e.request.method !== 'GET') return;
  
  var url = new URL(e.request.url);
  if (url.origin !== self.location.origin) return;

  e.respondWith(
    caches.match(e.request).then(function (cachedResponse) {
      if (cachedResponse) {
        // Fetch fresh in background to update cache (stale-while-revalidate)
        fetch(e.request).then(function (networkResponse) {
          if (networkResponse.status === 200) {
            caches.open(CACHE_NAME).then(function (cache) {
              cache.put(e.request, networkResponse);
            });
          }
        }).catch(function() {
          /* Ignore background fetch errors (e.g. offline) */
        });
        return cachedResponse;
      }

      return fetch(e.request).then(function (networkResponse) {
        if (!networkResponse || networkResponse.status !== 200 || networkResponse.type !== 'basic') {
          return networkResponse;
        }
        var responseToCache = networkResponse.clone();
        caches.open(CACHE_NAME).then(function (cache) {
          cache.put(e.request, responseToCache);
        });
        return networkResponse;
      });
    })
  );
});
