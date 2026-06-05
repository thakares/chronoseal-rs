/* ============================================================
   ChronoSeal Website — diagrams.js
   Generates SVG architecture & protocol flow diagrams inline
   ============================================================ */

var ChronoDiagrams = (function () {
  'use strict';

  function drawArchitecture(containerId) {
    var el = document.getElementById(containerId);
    if (!el) return;

    var svg = '<svg viewBox="0 0 760 420" fill="none" xmlns="http://www.w3.org/2000/svg" style="width:100%;height:auto;max-width:760px">';

    /* Background */
    svg += '<rect width="760" height="420" rx="12" fill="#131620"/>';

    /* Browser box */
    svg += '<rect x="40" y="30" width="280" height="170" rx="8" fill="#1e2230" stroke="#2a2f40" stroke-width="1.5"/>';
    svg += '<text x="180" y="55" fill="#b0b2b8" font-size="11" font-weight="700" text-anchor="middle" font-family="Inter,sans-serif">BROWSER</text>';

    svg += '<rect x="60" y="70" width="110" height="36" rx="5" fill="#252a3a" stroke="#2a2f40"/>';
    svg += '<text x="115" y="93" fill="#e1e1e4" font-size="10" text-anchor="middle" font-family="Inter,sans-serif">WASM Runtime</text>';

    svg += '<rect x="190" y="70" width="110" height="36" rx="5" fill="#252a3a" stroke="#2a2f40"/>';
    svg += '<text x="245" y="93" fill="#e1e1e4" font-size="10" text-anchor="middle" font-family="Inter,sans-serif">heartbeat.js</text>';

    svg += '<rect x="60" y="120" width="110" height="36" rx="5" fill="#252a3a" stroke="#2a2f40"/>';
    svg += '<text x="115" y="143" fill="#e1e1e4" font-size="10" text-anchor="middle" font-family="Inter,sans-serif">Ed25519 Keys</text>';

    svg += '<rect x="190" y="120" width="110" height="36" rx="5" fill="#252a3a" stroke="#2a2f40"/>';
    svg += '<text x="245" y="143" fill="#e1e1e4" font-size="10" text-anchor="middle" font-family="Inter,sans-serif">Gene Engine</text>';

    svg += '<text x="180" y="185" fill="#6b6f7a" font-size="9" text-anchor="middle" font-family="Inter,sans-serif">Entropy · Signing · VM · Mutation</text>';

    /* Arrow: Browser → Server */
    svg += '<line x1="320" y1="115" x2="430" y2="115" stroke="#7b7e85" stroke-width="1.5" stroke-dasharray="6 3"/>';
    svg += '<polygon points="428,110 438,115 428,120" fill="#7b7e85"/>';
    svg += '<text x="375" y="105" fill="#6b6f7a" font-size="8" text-anchor="middle" font-family="Inter,sans-serif">POST /hb</text>';
    svg += '<text x="375" y="135" fill="#6b6f7a" font-size="8" text-anchor="middle" font-family="Inter,sans-serif">POST /init</text>';

    /* Server box */
    svg += '<rect x="440" y="30" width="280" height="170" rx="8" fill="#1e2230" stroke="#2a2f40" stroke-width="1.5"/>';
    svg += '<text x="580" y="55" fill="#b0b2b8" font-size="11" font-weight="700" text-anchor="middle" font-family="Inter,sans-serif">SERVER (chronoseal)</text>';

    svg += '<rect x="460" y="70" width="110" height="36" rx="5" fill="#252a3a" stroke="#2a2f40"/>';
    svg += '<text x="515" y="93" fill="#e1e1e4" font-size="10" text-anchor="middle" font-family="Inter,sans-serif">Axum Router</text>';

    svg += '<rect x="590" y="70" width="110" height="36" rx="5" fill="#252a3a" stroke="#2a2f40"/>';
    svg += '<text x="645" y="93" fill="#e1e1e4" font-size="10" text-anchor="middle" font-family="Inter,sans-serif">Rate Limiter</text>';

    svg += '<rect x="460" y="120" width="110" height="36" rx="5" fill="#252a3a" stroke="#2a2f40"/>';
    svg += '<text x="515" y="143" fill="#e1e1e4" font-size="10" text-anchor="middle" font-family="Inter,sans-serif">Verifier</text>';

    svg += '<rect x="590" y="120" width="110" height="36" rx="5" fill="#252a3a" stroke="#2a2f40"/>';
    svg += '<text x="645" y="143" fill="#e1e1e4" font-size="10" text-anchor="middle" font-family="Inter,sans-serif">Gene Engine</text>';

    svg += '<text x="580" y="185" fill="#6b6f7a" font-size="9" text-anchor="middle" font-family="Inter,sans-serif">Verify · Advance · Trust · CAS</text>';

    /* Shared box (bottom center) */
    svg += '<rect x="230" y="240" width="300" height="60" rx="8" fill="#1e2230" stroke="#b0b2b8" stroke-width="1" stroke-dasharray="4 3"/>';
    svg += '<text x="380" y="266" fill="#b0b2b8" font-size="11" font-weight="700" text-anchor="middle" font-family="Inter,sans-serif">shared crate</text>';
    svg += '<text x="380" y="284" fill="#6b6f7a" font-size="9" text-anchor="middle" font-family="Inter,sans-serif">Protocol · Hashing · Gene · VM · Constants</text>';

    /* Arrows to shared */
    svg += '<line x1="180" y1="200" x2="310" y2="244" stroke="#7b7e85" stroke-width="1" stroke-dasharray="4 3"/>';
    svg += '<line x1="580" y1="200" x2="450" y2="244" stroke="#7b7e85" stroke-width="1" stroke-dasharray="4 3"/>';

    /* Storage box */
    svg += '<rect x="440" y="340" width="280" height="55" rx="8" fill="#1e2230" stroke="#2a2f40" stroke-width="1.5"/>';
    svg += '<text x="580" y="365" fill="#b0b2b8" font-size="11" font-weight="700" text-anchor="middle" font-family="Inter,sans-serif">STORAGE</text>';
    svg += '<text x="580" y="382" fill="#6b6f7a" font-size="9" text-anchor="middle" font-family="Inter,sans-serif">SQLite In-Memory · SQLite Disk · Valkey</text>';

    /* Server → Storage arrow */
    svg += '<line x1="580" y1="200" x2="580" y2="340" stroke="#7b7e85" stroke-width="1.5" stroke-dasharray="6 3"/>';
    svg += '<polygon points="575,338 580,348 585,338" fill="#7b7e85"/>';

    /* CLI box */
    svg += '<rect x="40" y="340" width="170" height="55" rx="8" fill="#1e2230" stroke="#2a2f40" stroke-width="1.5"/>';
    svg += '<text x="125" y="365" fill="#b0b2b8" font-size="11" font-weight="700" text-anchor="middle" font-family="Inter,sans-serif">CLI</text>';
    svg += '<text x="125" y="382" fill="#6b6f7a" font-size="9" text-anchor="middle" font-family="Inter,sans-serif">status · health · metrics · stats</text>';

    svg += '</svg>';
    el.innerHTML = svg;
  }

  function drawProtocolFlow(containerId) {
    var el = document.getElementById(containerId);
    if (!el) return;

    var svg = '<svg viewBox="0 0 600 520" fill="none" xmlns="http://www.w3.org/2000/svg" style="width:100%;height:auto;max-width:600px">';
    svg += '<rect width="600" height="520" rx="12" fill="#131620"/>';

    /* Columns */
    svg += '<text x="150" y="30" fill="#b0b2b8" font-size="12" font-weight="700" text-anchor="middle" font-family="Inter,sans-serif">Browser</text>';
    svg += '<text x="450" y="30" fill="#b0b2b8" font-size="12" font-weight="700" text-anchor="middle" font-family="Inter,sans-serif">Server</text>';

    /* Lifelines */
    svg += '<line x1="150" y1="44" x2="150" y2="500" stroke="#2a2f40" stroke-width="1.5"/>';
    svg += '<line x1="450" y1="44" x2="450" y2="500" stroke="#2a2f40" stroke-width="1.5"/>';

    var y = 70;
    var arrows = [
      { from: 150, to: 450, label: 'Generate Ed25519 keypair', note: '', dir: 'self' },
      { from: 150, to: 450, label: 'POST /init { public_key }', note: '', dir: 'right' },
      { from: 450, to: 150, label: '{ session_id, salt, opcodes, gene_size, mutation_order }', note: '', dir: 'left' },
      { from: 150, to: 450, label: 'Execute VM program', note: '', dir: 'self' },
      { from: 150, to: 450, label: 'Collect entropy + sign payload', note: '', dir: 'self' },
      { from: 150, to: 450, label: 'Preview gene commitment', note: '', dir: 'self' },
      { from: 150, to: 450, label: 'POST /hb { session_id, hash, signature, gene_commitment, … }', note: '', dir: 'right' },
      { from: 450, to: 150, label: 'Verify chain + signature + gene + trust', note: '', dir: 'selfr' },
      { from: 450, to: 150, label: '{ next_salt, next_mutation_step, next_mutation_order }', note: 'accepted', dir: 'left' },
      { from: 450, to: 150, label: '{ status: "ok" }', note: 'rejected (silent)', dir: 'left' },
    ];

    for (var i = 0; i < arrows.length; i++) {
      var a = arrows[i];
      if (a.dir === 'right') {
        svg += '<line x1="155" y1="' + y + '" x2="445" y2="' + y + '" stroke="#7b7e85" stroke-width="1.2"/>';
        svg += '<polygon points="443,' + (y - 4) + ' 450,' + y + ' 443,' + (y + 4) + '" fill="#7b7e85"/>';
        svg += '<text x="300" y="' + (y - 8) + '" fill="#e1e1e4" font-size="8.5" text-anchor="middle" font-family="Inter,sans-serif">' + a.label + '</text>';
      } else if (a.dir === 'left') {
        svg += '<line x1="445" y1="' + y + '" x2="155" y2="' + y + '" stroke="#7b7e85" stroke-width="1.2"/>';
        svg += '<polygon points="157,' + (y - 4) + ' 150,' + y + ' 157,' + (y + 4) + '" fill="#7b7e85"/>';
        svg += '<text x="300" y="' + (y - 8) + '" fill="#e1e1e4" font-size="8.5" text-anchor="middle" font-family="Inter,sans-serif">' + a.label + '</text>';
        if (a.note) {
          svg += '<text x="300" y="' + (y + 14) + '" fill="#6b6f7a" font-size="8" text-anchor="middle" font-style="italic" font-family="Inter,sans-serif">' + a.note + '</text>';
          y += 8;
        }
      } else if (a.dir === 'self') {
        svg += '<rect x="60" y="' + (y - 12) + '" width="180" height="22" rx="4" fill="#252a3a" stroke="#2a2f40"/>';
        svg += '<text x="150" y="' + (y + 3) + '" fill="#9a9da6" font-size="8.5" text-anchor="middle" font-family="Inter,sans-serif">' + a.label + '</text>';
      } else if (a.dir === 'selfr') {
        svg += '<rect x="360" y="' + (y - 12) + '" width="180" height="22" rx="4" fill="#252a3a" stroke="#2a2f40"/>';
        svg += '<text x="450" y="' + (y + 3) + '" fill="#9a9da6" font-size="8.5" text-anchor="middle" font-family="Inter,sans-serif">' + a.label + '</text>';
      }
      y += 44;
    }

    svg += '</svg>';
    el.innerHTML = svg;
  }

  /* ---- Auto-init ---- */
  function init() {
    drawArchitecture('diagram-architecture');
    drawProtocolFlow('diagram-protocol');
  }

  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', init);
  } else {
    init();
  }

  return {
    drawArchitecture: drawArchitecture,
    drawProtocolFlow: drawProtocolFlow
  };
})();
