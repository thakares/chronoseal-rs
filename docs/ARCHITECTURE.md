# ChronoSeal Architecture

## Overview

ChronoSeal is a stateless, cryptographic browser-attestation system. Its primary
goal is to make automated scraping and AI-driven crawling computationally
expensive and operationally complex, while remaining completely invisible to
real human users.

The design is inspired by the heartbeat model used in IoT firmware: a device
must continuously emit authenticated, chained proofs of liveness or the session
is invalidated. ChronoSeal applies this model to browser sessions.

---

## Core Principles

| Principle | Implementation |
|---|---|
| Stateless per request | Only `session_id` is sent; all state lives server-side in SQLite |
| Cryptographic continuity | Blake3 hash chain — each heartbeat references and extends the previous |
| Secret isolation | Ed25519 private key generated inside WASM, never serialised to JS |
| Silent failure | All rejections return `{"status":"ok"}` — indistinguishable from success |
| Behavioral validation | Mouse entropy, speed, and pause patterns validated server-side |
| Frictionless to humans | No CAPTCHA, no visible UI, zero interaction required |

---

## System Components

```
┌─────────────────────────────────────────────────────────────┐
│  Browser                                                    │
│                                                             │
│  ┌──────────────┐    ┌───────────────────────────────────┐  │
│  │  JavaScript  │    │  WASM Module (antibot_wasm)       │  │
│  │              │    │                                   │  │
│  │  heartbeat   │◄──►│  crypto.rs   — Ed25519 keypair    │  │
│  │  entropy     │    │  vm.rs       — stack machine      │  │
│  │  transport   │    │  (private key never leaves here)  │  │
│  └──────────────┘    └───────────────────────────────────┘  │
└───────────────────────────┬─────────────────────────────────┘
                            │ HTTPS
                            ▼
┌─────────────────────────────────────────────────────────────┐
│  Server (Axum / Tokio)                                      │
│                                                             │
│  POST /init ──► session.rs ──► storage.rs (SQLite)          │
│  POST /hb   ──► session.rs                                  │
│                   ├── crypto.rs     (Ed25519 verify)        │
│                   ├── trust.rs      (mouse entropy)         │
│                   ├── fingerprint   (browser signals)       │
│                   └── vm.rs         (opcode generation)     │
│                                                             │
│  Background: cleanup.rs (session expiry + RL eviction)      │
└─────────────────────────────────────────────────────────────┘
```

---

## Workspace Layout

```
chronoseal-rs/
│
├── shared/                  Shared between server and WASM
│   ├── src/constants.rs     All tunable parameters
│   ├── src/protocol.rs      Request/response types (serde)
│   └── src/hashing.rs       Blake3 hash-chain primitives
│
├── server/                  Axum HTTP server
│   ├── src/main.rs          Router, state init, background tasks
│   ├── src/routes/
│   │   ├── init.rs          POST /init handler
│   │   └── heartbeat.rs     POST /hb handler
│   ├── src/session.rs       Session lifecycle: create + verify
│   ├── src/crypto.rs        Ed25519 signature verification
│   ├── src/trust.rs         Behavioral signal validation
│   ├── src/fingerprint.rs   Browser fingerprint sanity checks
│   ├── src/vm.rs            Random opcode program generator
│   ├── src/ratelimit.rs     Token-bucket rate limiter
│   ├── src/cleanup.rs       Background expiry + eviction loop
│   ├── src/storage.rs       SQLite schema init + time utilities
│   └── src/middleware.rs    Request logging
│
├── wasm/                    Rust → WASM client module
│   ├── src/lib.rs           Module declarations
│   ├── src/crypto.rs        Keypair generation, signing, hashing
│   └── src/vm.rs            Stack machine executor
│
└── frontend/                Vanilla JS glue
    ├── index.html           Demo page
    ├── main.js              Entry point
    ├── heartbeat.js         Session init + heartbeat loop
    ├── entropy.js           Mouse event collector
    └── transport.js         fetch() wrapper
```

---

## Session Lifecycle

### 1. Initialisation (`POST /init`)

```
Client                                    Server
  │                                          │
  │  WASM: generate Ed25519 keypair          │
  │  private key → thread_local storage      │
  │                                          │
  ├─── { public_key: hex } ─────────────────►│
  │                                          │  Generate session_id (32 bytes CSPRNG)
  │                                          │  Generate salt₀ (16 bytes CSPRNG)
  │                                          │  H₀ = Blake3(session_id ║ pub_key ║ salt₀)
  │                                          │  Generate random VM program (8–16 opcodes)
  │                                          │  Store: session_id, pub_key, salt₀, H₀
  │                                          │
  │◄── { session_id, salt, opcodes, H₀ } ────┤
  │                                          │
  │  Store: session_id, prevHash=H₀,         │
  │         currentSalt=salt₀, opcodes       │
```

### 2. Heartbeat (`POST /hb`)

```
Client                                    Server
  │                                          │
  │  Collect mouse events since last HB      │
  │  Execute VM opcodes → stack state        │
  │  Build signable payload (sorted JSON):   │
  │    entropyData, fingerprint, prevHash,   │
  │    sessionId, stackState, timestamp      │
  │  Sign with Ed25519 private key           │
  │                                          │
  ├─── { session_id, prev_hash, timestamp,   │
  │       entropy_data, stack_state,         │
  │       fingerprint, signature } ─────────►│
  │                                          │  Rate limit check
  │                                          │  Look up session by session_id
  │                                          │  Check expiry
  │                                          │  Verify Ed25519 signature
  │                                          │  Verify prev_hash == stored last_hash
  │                                          │  Verify timestamp within ±30s
  │                                          │  Validate mouse entropy (speed, pauses)
  │                                          │  Validate fingerprint signals
  │                                          │  Compute: H(n) = Blake3(salt ║ H(n-1) ║ …)
  │                                          │  Generate next_salt
  │                                          │  Update: last_hash=H(n), salt=next_salt
  │                                          │
  │◄── { status: "ok", next_salt } ──────────┤
  │                                          │
  │  sentSalt = currentSalt                  │
  │  currentSalt = next_salt                 │
  │  prevHash = Blake3(sentSalt ║ H(n-1) ║ … │  ← must mirror server computation
```

### 3. Session Expiry

Sessions expire after 30 minutes of inactivity. A background task runs every
60 seconds to delete expired rows from SQLite and evict stale rate-limiter
entries from memory.

---

## Hash Chain

The chain provides tamper-evidence: forging a valid heartbeat at position `n`
requires knowledge of `H(n-1)`, the current `salt`, and the Ed25519 private key.
None of these are available to an attacker who does not control the client WASM.

```
H(0) = Blake3( session_id ║ pub_key ║ salt₀ )

H(n) = Blake3(
    saltₙ₋₁           ← server-side only, rotated each heartbeat
    ║ H(n-1)           ← stored server-side, sent by client
    ║ timestamp        ← 8 bytes LE
    ║ Blake3(entropy)  ← hash of mouse event JSON
    ║ Blake3(stack)    ← hash of VM stack state JSON
)
```

Salt rotation means that even a full replay of a captured heartbeat is invalid
on the next cycle — the salt has changed.

---

## Stack Machine

The server generates a random program on session init. The client executes it
on every heartbeat. The resulting stack state is included in the signed payload
and the hash chain, making each heartbeat structurally unique.

### Instruction Set

| Opcode | Mnemonic | Stack effect | Description |
|--------|----------|-------------|-------------|
| `0x00` + 4 bytes | PUSH | `→ val` | Push 32-bit LE literal |
| `0x01` | ADD | `a b → a+b` | Wrapping addition |
| `0x02` | SUB | `a b → a-b` | Wrapping subtraction |
| `0x03` | MUL | `a b → a*b` | Wrapping multiplication |
| `0x04` | XOR | `a b → a^b` | Bitwise XOR |
| `0x05` | AND | `a b → a&b` | Bitwise AND |
| `0x06` | OR  | `a b → a\|b` | Bitwise OR |
| `0x07` | ROT | `a b → a.rotate_left(b%32)` | Bit rotation |
| `0x08` | NOT | `a → !a` | Bitwise NOT (unary) |
| `0x09` | HASH | `[…] → u32` | Blake3 of full stack → single u32 |

### Program Generation

The server generates programs with a depth-tracking algorithm that guarantees
at least 2 operands before any binary op is emitted. Programs are 8–16
instructions. The client VM halts gracefully on underflow — invalid opcodes
produce a partial stack state that still participates in the hash chain.

---

## Cryptographic Primitives

| Primitive | Usage |
|---|---|
| **Ed25519** (ed25519-dalek v2) | Client keypair; signs each heartbeat payload |
| **Blake3** | Hash chain; entropy hashing; stack hashing |
| **CSPRNG** (rand / getrandom) | Session ID, salt, VM opcodes |

### Canonical Signing Format

The signed payload is a JSON object with keys sorted **alphabetically**,
matching `JSON.stringify(obj, Object.keys(obj).sort())` on the client and
`BTreeMap` serialisation on the server:

```json
{
  "entropyData":  { "events": [ { "x": 123.0, "y": 456.0, "t": 1234.5 } ] },
  "fingerprint":  { "aspectRatio": "1.7777777778", "devicePixelRatio": "2", "hardwareConcurrency": 8 },
  "prevHash":     "a3f2…",
  "sessionId":    "9c1b…",
  "stackState":   { "stack": [2147483648], "ip": 12 },
  "timestamp":    1746700000000
}
```

---

## Behavioral Validation

### Mouse Entropy

| Check | Threshold | Rationale |
|---|---|---|
| Minimum events | ≥ 3 | Single-point or no-movement signals headless |
| Total distance | ≥ 10 px | Rules out stationary cursors |
| Average speed | ≤ 2.0 px/ms | Rules out programmatic linear sweeps |
| Pause count | ≥ 1 | Human movement includes micro-stops |

Speed is computed as `total_distance / total_elapsed_ms` over the event window.

### Fingerprint Signals

| Signal | Valid range | Rationale |
|---|---|---|
| Aspect ratio | 0.5 – 3.0 | Headless defaults are often 1:1 or extreme values |
| Device pixel ratio | 0.0 – 5.0 | Zero DPR is impossible on real hardware |
| Hardware concurrency | ≥ 1 | Zero is impossible; headless may report 0 |

---

## Rate Limiting

Per-session token bucket: 5 requests per 10-second window. Rate limit state
is held in a `HashMap<String, (u32, Instant)>` in process memory. The cleanup
loop calls `evict_stale()` every 60 seconds to prevent unbounded growth from
unique session IDs.

---

## Storage

ChronoSeal uses an **in-memory SQLite** database. All session state is lost on
server restart. Clients transparently re-initialise on the next page load.

```sql
CREATE TABLE IF NOT EXISTS sessions (
    session_id    TEXT     PRIMARY KEY,
    public_key    BLOB     NOT NULL,
    salt          BLOB     NOT NULL,
    last_hash     BLOB     NOT NULL,
    chain_length  INTEGER  NOT NULL DEFAULT 1,
    created_at    INTEGER  NOT NULL,
    last_seen     INTEGER  NOT NULL,
    expires_at    INTEGER  NOT NULL
);
```

For persistence across restarts, replace `Connection::open_in_memory()` in
`server/src/storage.rs` with `Connection::open("/var/lib/chronoseal/sessions.db")`.

---

## Threat Model

### What ChronoSeal raises the cost of

- **Playwright / Puppeteer Stealth** — mouse entropy validation detects absent
  or synthetic movement
- **Replay attacks** — hash chain + salt rotation invalidates captured payloads
  on the next cycle
- **Signature forgery** — Ed25519 private key is generated inside WASM and
  never exposed to the JS context
- **Clock manipulation** — server enforces ±30s timestamp window
- **Credential sharing** — keypair is generated fresh on every page load;
  session is bound to that keypair
- **Traffic analysis** — all responses return `{"status":"ok"}`; rejections
  are indistinguishable from successes

### What ChronoSeal does not prevent

- An attacker running a real browser with a real mouse on real hardware
- A sufficiently motivated adversary who reverse-engineers the WASM, replicates
  the hash chain, and synthesises mouse events
- Server-side data exfiltration once a valid session is established

ChronoSeal is a **cost-raising** mechanism, not an impenetrable barrier.
The goal is to make scraping at scale impractical, not impossible.
