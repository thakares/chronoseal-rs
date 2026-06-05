/* ============================================================
   ChronoSeal Website — pwa.js
   Progressive Web App: service worker registration + install
   ============================================================ */

(function () {
  'use strict';

  if ('serviceWorker' in navigator) {
    window.addEventListener('load', function () {
      navigator.serviceWorker.register('/sw.js').then(function (reg) {
        /* Update found — silent refresh */
        reg.addEventListener('updatefound', function () {
          var worker = reg.installing;
          if (!worker) return;
          worker.addEventListener('statechange', function () {
            if (worker.state === 'activated' && navigator.serviceWorker.controller) {
              /* New version available — user can refresh */
            }
          });
        });
      }).catch(function () {
        /* SW registration failed — app still works */
      });
    });
  }

  /* ---- Installable PWA banner (A2HS) ---- */
  var deferredPrompt = null;

  window.addEventListener('beforeinstallprompt', function (e) {
    e.preventDefault();
    deferredPrompt = e;
    /* Could show a custom install banner here */
  });

  window.addEventListener('appinstalled', function () {
    deferredPrompt = null;
  });

})();
