# ChronoSeal Threat Model

ChronoSeal is a cost-raising browser attestation layer. It makes replay, stale state reuse, and incomplete automation more expensive by requiring signed, continuous, deterministic browser-side state progression.

It is not a perfect bot blocker, CAPTCHA replacement, hardware attestation system, fraud engine, or identity provider.

## Security Objectives

ChronoSeal aims to:

- reject stale or replayed heartbeat payloads
- reject heartbeats that do not maintain the server-issued mutation sequence
- bind heartbeat payloads to a browser-local Ed25519 session key
- make basic HTTP clients insufficient
- make browser automation maintain multiple synchronized state channels
- avoid detailed rejection feedback
- preserve privacy by avoiding persistent user identity state

## Protected Assets

| Asset | Protection focus |
|---|---|
| Protected page/API access | Require live attestation before allowing continued access |
| Session continuity | Ensure each accepted heartbeat advances from the last accepted state |
| Server compute | Rate-limit and reject invalid clients without expensive application work |
| Protocol state | Protect hash-chain, salt, and mutation progression |
| User privacy | Avoid long-term tracking and detailed failure disclosure |

## Trust Assumptions

ChronoSeal assumes:

- the server host and daemon process are trusted
- storage is trusted for session continuity
- TLS protects traffic in production
- browser clients can run JavaScript and WASM
- operators configure reverse proxy, filesystem permissions, and logs appropriately

ChronoSeal does not assume:

- the browser is honest
- WASM is a secure enclave
- mouse data proves human presence
- fingerprint values are unforgeable
- attackers cannot run a full browser

## Attacker Levels

### Level 1: Commodity HTTP Client

Examples:

- `curl`
- `requests`
- scraper scripts without browser or WASM execution

Expected result:

- cannot produce valid signatures
- cannot maintain hash-chain state
- cannot execute mutation preview
- cannot produce accepted heartbeats

### Level 2: Basic Headless Browser

Examples:

- Playwright
- Puppeteer
- Selenium

Expected result:

- can load JavaScript and WASM
- must preserve keypair, hash chain, salt, VM, and mutation state
- must generate plausible timing and mouse event windows
- silent rejection complicates debugging and scaling

### Level 3: Stealth Automation

Examples:

- patched browser runtime
- synthetic event generation
- custom protocol client with WASM or Rust reimplementation

Expected result:

- can attempt full protocol implementation
- must still match canonical signing, hash progression, mutation parity, and timing
- must handle changing server-issued mutation programs
- receives limited failure feedback

### Level 4: Resourced Browser Farm

Examples:

- real browsers
- realistic input devices
- human-assisted workflows
- distributed session management

Expected result:

- ChronoSeal raises cost and complexity
- ChronoSeal does not claim complete prevention
- additional application-level controls are required

## Attacker Classification Boundaries

### Protected
*   **Commodity Scrapers:** Simple HTTP clients (`curl`, Python `requests`, Go HTTP clients) that cannot execute JavaScript or WebAssembly.
*   **Simple Replay Attackers:** Intercepted heartbeat payloads cannot be reused because of the strict hash-chain sequencing and salt rotation.
*   **Signature Forgers:** Heartbeats without the session's private key will fail Ed25519 verification.

### Partially Protected
*   **Headless Automation (Puppeteer, Playwright):** Attackers must load the WASM runtime, execute the VM instructions, calculate gene mutations, and simulate realistic human mouse interactions. This significantly increases CPU and system memory overhead, reducing the scale of bot operations.
*   **Stealth Automation Frameworks:** Advanced frameworks must maintain state sync across multiple heartbeat cycles, exposing them to timing detection.

### Unprotected
*   **WASM Key Extraction:** A reverse engineer with full browser process control can extract the private key from WASM memory.
*   **Malware Operators:** Keyloggers, screen scrapers, or memory dumpers operating at the OS level are outside the application trust boundary.
*   **MITM Interceptors (without TLS):** Plaintext traffic can be intercepted. (TLS termination is assumed).
*   **Insiders / Storage Tampering:** Attackers with direct write access to the SQLite database or Valkey instance can forge or hijack active session states.

## Attack Vectors and Mitigations

### Replay

Attack: resend a previously accepted heartbeat.

Mitigations:

- stored `last_hash` must match request `prev_hash`
- accepted heartbeats rotate salt
- mutation step advances after acceptance
- timestamp drift is bounded

### Signature Forgery

Attack: submit a heartbeat without the browser session private key.

Mitigations:

- Ed25519 signature over canonical payload
- public key registered during `/init`
- signature verified on every heartbeat
- signature covers mutation step and gene commitment

### Hash-Chain Desynchronization

Attack: submit a heartbeat from stale client state.

Mitigations:

- server compares request `prev_hash` to stored `last_hash`
- server computes the next hash only after all validation passes
- rejected heartbeats do not advance server state

### Mutation Tampering

Attack: forge or skip synthetic gene mutations.

Mitigations:

- server stores the pending mutation program
- request must include the expected `mutation_step`
- server applies the mutation independently
- commitment includes candidate gene state, `session_id`, and step
- mismatch causes silent rejection

### Session Identifier Theft

Attack: reuse a stolen `session_id`.

Mitigations:

- `session_id` alone is insufficient
- attacker also needs current private key, hash state, salt, mutation step, and mutation state
- stale attempts fail after the real session advances

### Failure Oracle Probing

Attack: send malformed requests and inspect responses to infer validation rules.

Mitigations:

- heartbeat semantic failures return `200 OK` with `{"status":"ok"}`
- accepted heartbeats are distinguished only by next-state fields
- detailed validation errors are not returned to the client

### Storage Tampering

Attack: alter persisted session state.

Mitigations:

- run the daemon under a dedicated user
- restrict SQLite database permissions
- protect Valkey behind trusted network boundaries
- use normal host hardening and backups where persistence matters

Storage is trusted. If an attacker can modify storage, they can affect session continuity.

## Behavioral Checks

ChronoSeal validates:

- minimum event count
- minimum movement distance
- maximum average speed
- pause count
- timestamp drift
- basic fingerprint field ranges

These checks are cost signals. They are not proof of humanity and should not be the only security layer for high-risk actions.

## Privacy Constraints

ChronoSeal intentionally avoids:

- persistent user identifiers
- browser history collection
- device fingerprint databases
- cross-session identity graphs
- long-term behavioral profiles

Session data is short-lived by default. Persistent storage is operator-selected through `sqlite-in-disk` or `valkey`.

## Limitations

ChronoSeal does not protect against:

- real users intentionally automating or abusing access
- complete browser farms with realistic input
- compromised server hosts
- tampered storage
- server-side application vulnerabilities
- credential theft outside ChronoSeal
- policy decisions that require identity, risk scoring, or business context

## Operational Security

Recommended:

- serve all traffic over HTTPS
- keep `/init` and `/hb` same-origin with protected content when possible
- run behind a reverse proxy
- keep debug logs disabled in production
- protect storage and log directories
- monitor health and metrics
- use `sqlite-in-memory` for ephemeral sessions
- use `sqlite-in-disk` or `valkey` only when persistence is required

## Disclosure

See [../SECURITY.md](../SECURITY.md) for the vulnerability disclosure policy.
