# ChronoSeal — Threat Model

## Purpose

This document defines what ChronoSeal is designed to protect against, what
it explicitly does not protect against, and the reasoning behind each
design decision in security terms.

ChronoSeal is a **cost-raising mechanism**. It does not claim to make
automated access impossible. It makes automated access expensive, complex
to maintain, and operationally fragile at scale.

---

## Assets Being Protected

| Asset | Description |
|---|---|
| Web page content | HTML, rendered data, scraped text |
| API responses | JSON endpoints that serve structured data |
| Server compute | CPU and bandwidth consumed by automated clients |
| Rate-limited resources | Endpoints with per-user quotas |
| Behavioral analytics | Metrics polluted by bot traffic |

---

## Attacker Profiles

### Level 1 — Script Kiddie / Commodity Scraper

**Tools:** `curl`, `requests`, `scrapy`, simple HTTP clients.  
**Capability:** No browser environment. Cannot execute JavaScript or WASM.  
**ChronoSeal response:** Session never initialises. No `session_id` is ever
presented to `/hb`. Content gated behind session validation is never served.

### Level 2 — Headless Browser Operator

**Tools:** Playwright, Puppeteer, Selenium, undetected-chromedriver.  
**Capability:** Full browser environment. Can execute JavaScript and WASM.
Cannot easily synthesise realistic mouse entropy or maintain hash chain state
across concurrent sessions.  
**ChronoSeal response:** Mouse entropy validation rejects absent or synthetic
movement. Hash chain requires per-session state synchronisation. Scaling to
hundreds of concurrent sessions requires proportional infrastructure.

### Level 3 — Stealth Automation

**Tools:** Puppeteer Stealth, rebrowser-patches, custom CDP clients with
evasion patches.  
**Capability:** Patches `navigator.webdriver`, spoofs browser fingerprints,
can inject synthetic mouse events. May partially pass behavioral checks.  
**ChronoSeal response:** Ed25519 signature over the full payload (including
behavioral state and VM execution result) means the attacker must also
correctly execute the WASM program and maintain chain continuity. The private
key is generated fresh per page load and never exposed — it cannot be
extracted from a legitimate session and reused.

### Level 4 — Sophisticated Adversary

**Tools:** Full browser farm with real input devices, WASM reverse engineering,
custom chain maintenance infrastructure.  
**Capability:** Can pass all current ChronoSeal checks given sufficient
engineering effort.  
**ChronoSeal response:** Significantly increases operational cost. A browser
farm with real input devices costs orders of magnitude more than a commodity
scraper fleet. ChronoSeal is not designed to stop this attacker — no client-
side protection can.

---

## Attack Vectors and Mitigations

### Replay Attack

**Attack:** Capture a valid heartbeat payload and retransmit it.  
**Mitigation:**
- Timestamp window (±30 seconds): replayed payloads are rejected after 30s.
- Hash chain: each heartbeat must present `H(n-1)` matching the server's
  stored state. A replayed heartbeat presents a stale hash that no longer
  matches after one successful heartbeat has advanced the chain.

### Signature Forgery

**Attack:** Construct a valid-looking heartbeat payload without the private key.  
**Mitigation:** Ed25519 with 128-bit security. The private key is generated
inside WASM `thread_local` memory, never serialised, never passed to
JavaScript, never transmitted. Forgery requires breaking Ed25519 or
extracting the key from WASM memory — neither is practical.

### Key Extraction

**Attack:** Inspect WASM linear memory to extract the private signing key.  
**Mitigation:** The key is stored in a Rust `thread_local! { RefCell<Option<SigningKey>> }`.
It has no exported symbol and is not referenced by any exported WASM function
that returns raw memory. An attacker with full DevTools access to the WASM
memory can extract it from one session, but it is useless for other sessions
(fresh keypair per page load) and expires with the session.

### Hash Chain Forgery

**Attack:** Compute a valid `H(n)` without the server-side salt.  
**Mitigation:** Each chain link incorporates `saltₙ₋₁`, which is a 16-byte
random value known only to the server and returned (once) in the heartbeat
response. An attacker cannot compute `H(n+1)` without first receiving
`saltₙ` from a successful heartbeat response, which requires a valid signature
and all other checks to pass.

### Session Hijacking

**Attack:** Steal a `session_id` and use it from a different client.  
**Mitigation:** `session_id` alone is insufficient — the attacker also needs
the private key (to produce valid signatures) and the current chain state
(to present the correct `prev_hash`). All three are required simultaneously.

### Enumeration of Validation Rules

**Attack:** Send malformed heartbeats and analyse error responses to map
validation logic.  
**Mitigation:** All failure paths return `{"status":"ok"}` with no `next_salt`.
There is no error code, no error message, and no status difference between
a rate limit hit, an invalid signature, a broken chain, and a behavioral
rejection.

### DoS via Session Flooding

**Attack:** Open thousands of sessions to exhaust the rate limiter's HashMap
memory.  
**Mitigation:** Rate limiter entries are evicted every 60 seconds by the
cleanup task. Each entry is a small `(u32, Instant)` tuple; even at 100,000
concurrent fake sessions, the HashMap occupies roughly 10–15 MB, which is
well within normal server memory budgets. Sessions themselves expire after 30
minutes of inactivity and are purged from SQLite.

### Clock Manipulation

**Attack:** Manipulate the client's `Date.now()` to bypass the timestamp
window.  
**Mitigation:** The timestamp is included in the signed payload. Manipulating
it requires also forging the signature. The server validates against its own
clock — client-side clock manipulation cannot help without the private key.

### Synthetic Mouse Events

**Attack:** Inject programmatic `mousemove` events via `dispatchEvent` or
CDP input simulation.  
**Mitigation:** Synthetic events often fail the pause check (no natural dwell
periods), produce unrealistically uniform speed profiles, or fail the minimum
distance threshold. Generating convincingly human mouse traces at scale
requires either real input devices or sophisticated probabilistic models —
both significantly increase operational cost.

---

## What ChronoSeal Does Not Protect Against

| Limitation | Explanation |
|---|---|
| Real browsers with real users acting as bots | A human operating a browser manually is indistinguishable from a legitimate visitor. ChronoSeal cannot address this. |
| Server-side vulnerabilities | ChronoSeal is a client attestation layer. It does not protect the server from injection, authentication bypass, or other backend vulnerabilities. |
| Highly resourced nation-state actors | Out of scope for a client-side protection layer. |
| Content visible before session establishment | If the protected content is rendered before the first heartbeat, it can be scraped without a session. Gate content on session validity server-side. |
| Perfect bot prevention | No client-side mechanism can be. WASM can be reverse engineered. ChronoSeal raises cost, not an impenetrable barrier. |

---

## Operational Security Notes

### Log Level

Do not run with `RUST_LOG=debug` in production. The debug log includes
`session_id` values, which are sensitive identifiers. Use `warn` or `info`.

### CORS Policy

The default `CorsLayer::permissive()` is suitable for development only.
In production, restrict allowed origins to your own domain:

```rust
CorsLayer::new()
    .allow_origin("https://your.domain.com".parse::<HeaderValue>().unwrap())
    .allow_methods([Method::POST])
    .allow_headers([header::CONTENT_TYPE])
```

### TLS

Serve exclusively over TLS 1.3. The heartbeat payload contains timestamps
and behavioral signals. While each payload is signed and cannot be forged,
plaintext transmission leaks behavioral patterns and timing information that
could assist a sophisticated attacker.

### In-Memory SQLite

All session state is lost on server restart. This is intentional — there is
no persistent state to steal. Clients transparently re-initialise. If your
deployment restarts frequently (e.g. rolling deploys), sessions will be lost
more often; tune `HEARTBEAT_MIN_INTERVAL_MS` and `EXPIRATION_MINUTES`
accordingly so clients recover quickly.

---

## Security Disclosure

See [SECURITY.md](../SECURITY.md) for the vulnerability disclosure policy
and contact details.
