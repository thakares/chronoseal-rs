# ChronoSeal — Architecture

## Overview

ChronoSeal is a stateless, cryptographic browser attestation framework. Its
purpose is to make automated clients (headless browsers, AI scrapers, API
harvesters) computationally expensive and operationally complex to operate,
while remaining completely invisible to real human users.

The design is inspired by the heartbeat model used in embedded IoT firmware:
a device that stops sending signed, chained attestations is assumed to be
offline or compromised. ChronoSeal applies the same principle to browser
sessions.

---

## Design Principles

**Stateless per request.** The server carries no per-request state beyond what
is stored in SQLite keyed on `session_id`. Every HTTP request is independently
verifiable.

**Silent failure.** Validation failures never return an error status or an
error body. The server always responds `{"status":"ok"}` and simply omits
`next_salt`. The client degrades gracefully. Attackers cannot enumerate
validation rules by probing error responses.

**Private key isolation.** The Ed25519 signing key is generated inside the
WASM module and never serialised, never exposed to the JavaScript environment,
and never transmitted. It exists only in WASM linear memory for the lifetime
of the page.

**Layered validation.** A heartbeat must pass five independent checks: session
existence, expiry, signature, hash chain, and behavioral signals. Bypassing
one layer is not sufficient.

**Cost asymmetry.** Each heartbeat requires a real browser environment, mouse
activity, correct WASM execution, chain state synchronisation, and a valid
Ed25519 signature over a time-windowed payload. For an automated client, the
synchronisation burden alone makes scaled operation expensive.

---

## Component Map

```
┌─────────────────────────────────────────────────────────┐
│  Browser                                                │
│                                                         │
│  ┌─────────────┐   ┌──────────────┐   ┌─────────────┐  │
│  │  entropy.js │   │ heartbeat.js │   │ transport.js│  │
│  │             │   │              │   │             │  │
│  │ mousemove   │──►│ orchestrates │──►│ fetch POST  │  │
│  │ event ring  │   │ init + HB    │   │ /init  /hb  │  │
│  └─────────────┘   └──────┬───────┘   └─────────────┘  │
│                           │                             │
│                    ┌──────▼───────────────────────┐     │
│                    │  WASM Module (antibot_wasm)   │     │
│                    │                               │     │
│                    │  crypto.rs      vm.rs         │     │
│                    │  ├ generate_keypair()          │     │
│                    │  ├ sign_message()              │     │
│                    │  ├ compute_next_hash()         │     │
│                    │  └ run_program()               │     │
│                    └───────────────────────────────┘     │
└─────────────────────────────────────────────────────────┘
                          │ HTTPS
┌─────────────────────────▼───────────────────────────────┐
│  Server (Axum)                                          │
│                                                         │
│  routes/init.rs          routes/heartbeat.rs            │
│       │                          │                      │
│       └──────────┬───────────────┘                      │
│                  ▼                                       │
│            session.rs                                   │
│            ├ create_session()                           │
│            └ verify_heartbeat()                         │
│                  │                                      │
│       ┌──────────┼──────────────┐                       │
│       ▼          ▼              ▼                       │
│  crypto.rs   trust.rs    fingerprint.rs                 │
│  (sig verify) (mouse     (aspect ratio,                 │
│               speed)      DPR, HW conc.)                │
│       │                                                 │
│       ▼                                                 │
│  shared::hashing  (Blake3 hash chain)                   │
│       │                                                 │
│       ▼                                                 │
│  storage.rs  (in-memory SQLite)                         │
│                                                         │
│  ratelimit.rs   cleanup.rs   vm.rs   middleware.rs      │
└─────────────────────────────────────────────────────────┘
```

---

## Session Lifecycle

### 1. Initialisation — `POST /init`

```
Client                                  Server
  │                                        │
  │  generate Ed25519 keypair (in WASM)    │
  │  pub_key = verifying_key.to_bytes()    │
  │                                        │
  ├─── { public_key: hex(pub_key) } ──────►│
  │                                        │  session_id = rand::random::<[u8;32]>()
  │                                        │  salt₀      = rand::random::<[u8;16]>()
  │                                        │  H(0) = Blake3(session_id║pub_key║salt₀)
  │                                        │  opcodes = generate_random_program(8..=16)
  │                                        │  INSERT INTO sessions …
  │                                        │
  │◄── { session_id, salt, opcodes_b64,   │
  │      initial_hash, expires_at } ───────┤
  │                                        │
  │  prevHash    = initial_hash            │
  │  currentSalt = salt                    │
  │  opcodesB64  = opcodes_b64             │
```

### 2. Heartbeat — `POST /hb`

Fired every 12–25 seconds with uniform random jitter.

```
Client                                  Server
  │                                        │
  │  stackState = run_program(opcodesB64)  │
  │  events     = collectEntropy(lastTime) │
  │  ts         = Date.now()               │
  │                                        │
  │  signable = {                          │
  │    entropyData, fingerprint,           │  ← keys sorted alphabetically
  │    prevHash, sessionId,                │
  │    stackState, timestamp               │
  │  }                                     │
  │  sig = sign_message(                   │
  │    JSON.stringify(signable, keys.sort))│
  │                                        │
  ├─── { session_id, prev_hash, timestamp,│
  │      entropy_data, stack_state,        │
  │      fingerprint, signature } ────────►│
  │                                        │  1. Rate limit check
  │                                        │  2. Lookup session, check expiry
  │                                        │  3. Verify Ed25519 signature
  │                                        │  4. Verify hash chain continuity
  │                                        │  5. Validate timestamp window ±30s
  │                                        │  6. Validate mouse behavior
  │                                        │  7. Validate fingerprint signals
  │                                        │  8. Compute H(n), rotate salt
  │                                        │  9. UPDATE sessions …
  │                                        │
  │◄── { status: "ok", next_salt } ────────┤
  │                                        │
  │  sentSalt    = currentSalt  ◄── captured BEFORE rotation
  │  currentSalt = next_salt               │
  │  prevHash    = compute_next_hash(      │
  │    prevHash, ts, entropy,              │
  │    stackState, sentSalt)               │
```

### 3. Failure Path

On any validation failure the server returns `{"status":"ok"}` with no
`next_salt`. The client logs a warning and continues scheduling heartbeats.
The chain is broken — subsequent heartbeats will also fail silently.
No error is surfaced to the page or its visitors.

---

## Cryptographic Protocol

### Key Generation

```
Ed25519 keypair generated via ed25519-dalek + rand::thread_rng (OS-seeded)
Private key:  stored in WASM thread_local, never leaves WASM memory
Public key:   32 bytes, hex-encoded, sent to server at init
```

### Hash Chain

```
H(0) = Blake3( session_id ║ pub_key ║ salt₀ )

H(n) = Blake3(
    saltₙ₋₁                    ← server-side only, rotated each heartbeat
  ║ H(n-1)                     ← must match stored last_hash
  ║ timestamp_u64_le
  ║ Blake3( JSON(entropy_data) )
  ║ Blake3( JSON(stack_state)  )
)
```

Salt rotation means an attacker who intercepts a heartbeat cannot compute
future chain links without also intercepting every subsequent server response.

### Canonical Signing Payload

The signed message is a JSON object with top-level keys sorted alphabetically,
serialised with no extra whitespace:

```json
{
  "entropyData":  { "events": [{"t":…,"x":…,"y":…}] },
  "fingerprint":  { "aspectRatio":"…","devicePixelRatio":"…","hardwareConcurrency":… },
  "prevHash":     "hex…",
  "sessionId":    "hex…",
  "stackState":   { "ip":…,"stack":[…] },
  "timestamp":    1234567890123
}
```

The server reconstructs this using `std::collections::BTreeMap` (alphabetical
key order) before calling `VerifyingKey::verify_strict`. Any field mismatch,
key order difference, or whitespace difference causes a signature failure.

### Hashing Algorithm

Blake3 is used throughout: hash chain links, entropy data digest, stack state
digest, and the VM HASH opcode. Blake3 is chosen for speed in WASM,
resistance to length-extension attacks, and a clean Rust API.

---

## Stack Machine

The server generates a random program on session init. The client executes it
on every heartbeat and includes the resulting `StackState { stack, ip }` in
the signed payload. This ensures each heartbeat carries unique, verifiable
computation without additional round-trips.

### Instruction Set

| Opcode | Mnemonic | Operand       | Stack effect | Description |
|--------|----------|---------------|--------------|-------------|
| `0x00` | PUSH     | u32 (4B LE)   | +1           | Push literal |
| `0x01` | ADD      | —             | −1           | `a + b` wrapping |
| `0x02` | SUB      | —             | −1           | `a - b` wrapping |
| `0x03` | MUL      | —             | −1           | `a * b` wrapping |
| `0x04` | XOR      | —             | −1           | `a ^ b` |
| `0x05` | AND      | —             | −1           | `a & b` |
| `0x06` | OR       | —             | −1           | `a \| b` |
| `0x07` | ROT      | —             | −1           | `a.rotate_left(b % 32)` |
| `0x08` | NOT      | —             | 0            | `!a` (unary) |
| `0x09` | HASH     | —             | -(depth-1)   | Blake3 of all stack items → single u32 |

The generator ensures ≥ 2 items on the stack before any binary opcode.
NOT (0x08) does not change depth. HASH resets depth to 1.

---

## Behavioral Validation

### Mouse Entropy

Every heartbeat includes the mouse events collected since the previous
heartbeat. Server checks:

| Check | Threshold |
|---|---|
| Minimum event count | ≥ 3 |
| Minimum cumulative distance | ≥ 10 px |
| Maximum average speed | ≤ 2.0 px/ms (distance / elapsed ms) |
| Minimum pause count | ≥ 1 (movement < 0.2 px over > 50 ms) |

### Browser Fingerprint

| Signal | Valid range |
|---|---|
| `aspectRatio` (width / height) | 0.5 – 3.0 |
| `devicePixelRatio` | 0 < dpr ≤ 5.0 |
| `hardwareConcurrency` | ≥ 1 |

---

## Rate Limiting

Token bucket per `session_id`: 5 requests / 10-second window.
Stale entries evicted every 60 seconds by the cleanup task.
Rate-limited responses are indistinguishable from validation failures.

---

## SQLite Schema

```sql
CREATE TABLE IF NOT EXISTS sessions (
    session_id    TEXT     PRIMARY KEY,
    public_key    BLOB     NOT NULL,   -- 32-byte Ed25519 verifying key
    salt          BLOB     NOT NULL,   -- 16-byte current salt
    last_hash     BLOB     NOT NULL,   -- 32-byte Blake3 chain head
    chain_length  INTEGER  NOT NULL DEFAULT 1,
    created_at    INTEGER  NOT NULL,   -- Unix ms
    last_seen     INTEGER  NOT NULL,   -- Unix ms
    expires_at    INTEGER  NOT NULL    -- Unix ms
);
```

In-memory SQLite — all sessions lost on server restart by design.
Clients re-initialise transparently on the next page load.

---

## Threat Model

### In Scope

| Threat | Mitigation |
|---|---|
| Playwright / Puppeteer / Selenium | Mouse entropy + behavioral validation |
| Puppeteer Stealth, undetected-chromedriver | Signature over VM execution state |
| Heartbeat replay | Hash chain + ±30s timestamp window |
| Signature forgery | Private key isolated in WASM memory |
| Parallel session sharing | Each session bound to a unique keypair |
| Brute-forced session IDs | 256-bit random entropy |
| Flooding with fake session IDs | Rate limiter + periodic HashMap eviction |
| Traffic analysis | Uniform `{"status":"ok"}` on all failure paths |

### Out of Scope

| Threat | Reason |
|---|---|
| Real browser with real human input | Indistinguishable from a legitimate user |
| WASM reverse engineering | Obfuscation is not a security primitive |
| Server-side compromise | Outside the scope of client attestation |

ChronoSeal raises cost and complexity of automated access. It is not a
cryptographic proof of humanity and does not claim to be.

---

## Module Reference

| Path | Purpose |
|---|---|
| `shared/src/protocol.rs` | Shared types: `InitRequest`, `HeartbeatRequest`, `StackState`, … |
| `shared/src/hashing.rs` | `initial_hash`, `next_chain_hash`, `hash_stack` |
| `shared/src/constants.rs` | All tunable parameters |
| `server/src/routes/init.rs` | `POST /init` handler |
| `server/src/routes/heartbeat.rs` | `POST /hb` handler |
| `server/src/session.rs` | `create_session`, `verify_heartbeat` |
| `server/src/crypto.rs` | `verify_signature` — BTreeMap canonical JSON |
| `server/src/trust.rs` | `validate_mouse` — speed, distance, pauses |
| `server/src/fingerprint.rs` | `validate` — aspect ratio, DPR, HW concurrency |
| `server/src/vm.rs` | `generate_random_program` |
| `server/src/ratelimit.rs` | `RateLimiter::check`, `evict_stale` |
| `server/src/cleanup.rs` | Background loop: expire sessions + evict rate limiter |
| `server/src/storage.rs` | SQLite init, `current_time_ms` |
| `wasm/src/crypto.rs` | `generate_keypair`, `sign_message`, `compute_next_hash` |
| `wasm/src/vm.rs` | `run_program` — stack machine executor |
| `frontend/heartbeat.js` | Session init, heartbeat loop, chain advancement |
| `frontend/entropy.js` | Mouse event ring buffer, `collectEntropy` |
| `frontend/transport.js` | `sendRequest` fetch wrapper |
