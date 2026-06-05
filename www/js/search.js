/* ============================================================
   ChronoSeal Website — search.js
   Client-side full-text search across all pages
   ============================================================ */

var ChronoSearch = (function () {
  'use strict';

  /* ---- Search index ---- */
  var pages = [
    { title: 'Home',              url: 'index.html',        desc: 'ChronoSeal overview, features, and quick start',         keywords: 'home overview features quick start install' },
    { title: 'Architecture',      url: 'architecture.html',  desc: 'System architecture, crate layout, and data flow',       keywords: 'architecture crates workspace wasm shared server' },
    { title: 'Protocol',          url: 'protocol.html',      desc: 'Heartbeat protocol, hash chain, and mutation engine',     keywords: 'protocol heartbeat hash chain blake3 ed25519 mutation gene' },
    { title: 'API Reference',     url: 'api.html',           desc: 'HTTP endpoints: /init, /hb, /health, /metrics, /stats',  keywords: 'api http post init heartbeat health metrics stats json' },
    { title: 'Deployment',        url: 'deployment.html',    desc: 'Installation, systemd, Docker, nginx reverse proxy',     keywords: 'deploy install systemd docker nginx proxy production' },
    { title: 'Threat Model',      url: 'threat-model.html',  desc: 'Security threat model and defense boundaries',           keywords: 'threat model security attack replay automation defense' },
    { title: 'Performance',       url: 'performance.html',   desc: 'Performance tuning, benchmarks, and optimization',       keywords: 'performance tuning benchmark latency throughput' },
    { title: 'Philosophy',        url: 'philosophy.html',    desc: 'Design philosophy and engineering priorities',            keywords: 'philosophy design unix privacy operator control' },
    { title: 'Privacy Policy',    url: 'privacy.html',       desc: 'Data handling, storage, and privacy commitments',        keywords: 'privacy policy data collection storage session' },
    { title: 'Security',          url: 'security.html',      desc: 'Security assumptions, hardening, and response headers',  keywords: 'security assumptions headers hardening csp' },
    { title: 'Operations',        url: 'operations.html',    desc: 'Monitoring, health checks, metrics, and logging',        keywords: 'operations health status metrics stats logging monitor' },
    { title: 'Testing',           url: 'testing.html',       desc: 'Test suite, fuzzing, and validation coverage',           keywords: 'testing tests cargo fuzz validation fingerprint' },
    { title: 'Comparison',        url: 'comparison.html',    desc: 'ChronoSeal vs commercial anti-bot solutions',            keywords: 'comparison cloudflare akamai recaptcha datadome kasada' },
  ];

  var overlay = null;
  var input   = null;
  var results = null;

  function init() {
    overlay = document.querySelector('.search-overlay');
    input   = document.querySelector('.search-input');
    results = document.querySelector('.search-results');
    if (!overlay || !input || !results) return;

    input.addEventListener('input', debounce(onInput, 150));
    overlay.addEventListener('click', function (e) {
      if (e.target === overlay) close();
    });

    // Keyboard nav inside results
    input.addEventListener('keydown', function (e) {
      if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
        e.preventDefault();
        navigateResults(e.key === 'ArrowDown' ? 1 : -1);
      }
      if (e.key === 'Enter') {
        var sel = results.querySelector('.search-result.selected');
        if (sel) { window.location.href = sel.getAttribute('href'); close(); }
      }
    });
  }

  function open() {
    if (!overlay) init();
    if (!overlay) return;
    overlay.classList.add('open');
    input.value = '';
    results.innerHTML = renderHints();
    setTimeout(function () { input.focus(); }, 100);
  }

  function close() {
    if (overlay) overlay.classList.remove('open');
  }

  function onInput() {
    var q = input.value.trim().toLowerCase();
    if (!q) { results.innerHTML = renderHints(); return; }

    var matches = [];
    for (var i = 0; i < pages.length; i++) {
      var p = pages[i];
      var hay = (p.title + ' ' + p.desc + ' ' + p.keywords).toLowerCase();
      if (hay.indexOf(q) !== -1) {
        matches.push(p);
      }
    }

    if (matches.length === 0) {
      results.innerHTML = '<div class="search-empty">No results for &ldquo;' + escHtml(q) + '&rdquo;</div>';
      return;
    }

    var html = '';
    for (var j = 0; j < matches.length; j++) {
      var m = matches[j];
      html += '<a class="search-result' + (j === 0 ? ' selected' : '') + '" href="' + m.url + '">';
      html += '<div class="search-result-title">' + highlightMatch(m.title, q) + '</div>';
      html += '<div class="search-result-desc">' + highlightMatch(m.desc, q) + '</div>';
      html += '</a>';
    }
    results.innerHTML = html;
  }

  function navigateResults(dir) {
    var items = results.querySelectorAll('.search-result');
    if (!items.length) return;
    var idx = -1;
    for (var i = 0; i < items.length; i++) {
      if (items[i].classList.contains('selected')) { idx = i; break; }
    }
    if (idx >= 0) items[idx].classList.remove('selected');
    idx = Math.max(0, Math.min(items.length - 1, idx + dir));
    items[idx].classList.add('selected');
    items[idx].scrollIntoView({ block: 'nearest' });
  }

  function renderHints() {
    return '<div class="search-empty">Type to search documentation&hellip;<br><span style="font-size:.78rem;color:var(--text-muted)">↑↓ Navigate &middot; ↵ Open &middot; Esc Close</span></div>';
  }

  function highlightMatch(text, q) {
    var idx = text.toLowerCase().indexOf(q);
    if (idx === -1) return escHtml(text);
    return escHtml(text.substring(0, idx)) +
           '<mark style="background:rgba(176,178,184,.2);color:var(--text-primary);border-radius:2px;padding:0 2px">' +
           escHtml(text.substring(idx, idx + q.length)) + '</mark>' +
           escHtml(text.substring(idx + q.length));
  }

  function escHtml(s) {
    var div = document.createElement('div');
    div.appendChild(document.createTextNode(s));
    return div.innerHTML;
  }

  function debounce(fn, ms) {
    var t;
    return function () {
      clearTimeout(t);
      t = setTimeout(fn, ms);
    };
  }

  /* ---- Auto-init ---- */
  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', init);
  } else {
    init();
  }

  return { open: open, close: close };
})();
