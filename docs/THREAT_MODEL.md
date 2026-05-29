# ChronoSeal Threat Model

ChronoSeal is a cost-raising cryptographic attestation daemon. It increases the burden on automated clients while preserving privacy, determinism, and operational transparency.

## Purpose

ChronoSeal protects web resources by making browser automation and replay attacks more expensive and fragile. It is not intended to be a perfect bot blocker.

## Protected Assets

| Asset | Protection focus |
|---|---|
| Page content | Prevent automated scraping and replay of protected content |
| API responses | Reduce scripted access to sensitive endpoints |
| Server compute | Increase attacker resource costs |
| Session continuity | Enforce live session progression |
| Behavioral integrity | Validate plausible browser activity |

## Attacker Profiles

### Level 1 — Commodity Scraper

* Tools: `curl`, `requests`, headless HTTP clients
* Capability: no WASM execution, no browser engine

ChronoSeal response:

* cannot initialize a session
* no `session_id` is produced
* content remains protected behind the attestation layer

### Level 2 — Headless Browser Operator

* Tools: Playwright, Puppeteer, Selenium
* Capability: browser engine available, but automation is not indistinguishable from a real user

ChronoSeal response:

* mouse entropy and pause checks become active barriers
* hash chain continuity requires per-session state tracking
* synthetic heartbeats become expensive to maintain at scale

### Level 3 — Stealth Automation

* Tools: browser stealth plugins, CDP patching, synthetic event injection
* Capability: can execute JavaScript and WASM, may spoof some browser signals

ChronoSeal response:

* signature, hash chain, and mutation commitment require correct WASM execution
* private key is generated per page load and never exposes raw key material
* silent rejection hides validation rules from attacker feedback

### Level 4 — Sophisticated Operator

* Tools: real browser farms, hardware input devices, custom chain management
* Capability: high engineering investment and real device scale

ChronoSeal response:

* significantly increases operational cost and complexity
* forces a full protocol implementation rather than best-effort scraping
* is not designed to stop such adversaries completely

## Attack Vectors and Mitigations

### Replay Attack

**Attack:** resend a previously observed heartbeat.

**Mitigations:**

* timestamp window enforcement (±30 seconds)
* chained Blake3 hash continuity
* server-issued salt rotation
* mutation step progression

### Signature Forgery

**Attack:** forge a heartbeat without the private key.

**Mitigations:**

* Ed25519 signature over the canonical payload
* private key generated and stored inside WASM memory only
* signature verification occurs on every heartbeat

### Mutation Tampering

**Attack:** send an invalid or stale mutation commitment.

**Mitigations:**

* server recomputes the gene commitment from server-authored mutation orders
* heartbeat request includes `mutation_step` and `gene_commitment`
* mismatched commitment causes silent rejection

### Session Hijacking

**Attack:** steal a valid `session_id` and reuse it.

**Mitigations:**

* `session_id` alone is insufficient
* attacker also needs current `prev_hash` and private key
* keypair is generated per browser session in WASM

### Fingerprint Enumeration

**Attack:** probe the API with malformed requests to discover validation logic.

**Mitigations:**

* all invalid heartbeats return `{"status":"ok"}`
* no explicit error messages are exposed
* silent rejection removes oracle behavior

## Limitations

ChronoSeal does not protect against:

* real users intentionally acting as bots
* server-side application vulnerabilities
* full browser farm operators with real input devices
* persistent fingerprinting or identity profiling
* pre-signed session payload reuse after a legitimate success if the attacker also has the current salt and key

## Operational Security Notes

* Do not use `RUST_LOG=debug` in production; it may expose internal identifiers.
* Always serve ChronoSeal traffic over HTTPS.
* Use `sqlite-in-memory` for ephemeral sessions when persistence is not required.
* Use `sqlite-disk` or `valkey` when session state needs to survive restarts.

## Disclosure

See [SECURITY.md](../SECURITY.md) for the vulnerability disclosure policy.
