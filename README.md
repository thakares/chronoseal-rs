# ChronoSeal

**Cryptographic anti-automation and browser attestation framework built with Rust, WASM, and behavioral continuity verification.**

ChronoSeal makes it computationally expensive and operationally complex for AI scrapers, headless browsers, and automation tools to impersonate real users — while remaining completely invisible and frictionless to legitimate human visitors.

---

## How It Works

ChronoSeal establishes a continuous, cryptographically verifiable proof-of-presence for every browser session. It is inspired by the heartbeat model used in IoT firmware (ESP32-class devices): the client must keep emitting signed, chained attestations, or the session is silently invalidated.

```
Browser                                    Server
  │                                           │
  │  WASM loads, generates Ed25519 keypair    │
  │  Private key never leaves WASM memory     │
  │                                           │
  ├──── POST /init  { public_key } ──────────►│  Store session, salt, initial hash
  │◄─── { session_id, salt, opcodes, H0 } ────┤
  │                                           │
  │  Every 12–25s (jittered):                 │
  │  ┌─ Collect mouse entropy                 │
  │  ├─ Execute VM opcodes → stack state      │
  │  ├─ Compute H(n) = Blake3(H(n-1) ║ …)   │
  │  └─ Sign payload with Ed25519             │
  │                                           │
  ├──── POST /hb  { session_id, sig, … } ────►│  Verify sig → chain → behavior → fingerprint
  │◄─── { status, next_salt } ────────────────┤  Rotate salt, advance chain
  │                                           │
  │  On failure: server returns {"status":"ok"}│  Silent rejection — indistinguishable
```

---

## Security Model

### What ChronoSeal protects against

| Threat | Mechanism |
|---|---|
| Playwright / Puppeteer Stealth | Mouse entropy validation rejects synthetic or absent movement |
| Replay attacks | Hash chain — each heartbeat references the previous hash; old payloads are invalid |
| Signature forgery | Ed25519 private key generated inside WASM, never serialised or exposed to JS |
| Clock manipulation | Server enforces ±30s timestamp window |
| Credential sharing | Session is bound to a keypair generated fresh on every page load |
| Flooding with fake sessions | Per-session rate limiting (5 req / 10s); stale entries evicted every 60s |
| Passive analysis of traffic | Server always returns `{"status":"ok"}` — rejections are silent |

### What ChronoSeal does not claim

ChronoSeal is a cost-raising mechanism, not an impenetrable barrier. A sufficiently motivated adversary with a real browser, real input devices, and the patience to reverse the WASM can bypass it. The goal is to make scraping expensive and operationally complex enough to be impractical at scale.

---

## Architecture

```
chronoseal-rs/
├── shared/          Shared types, Blake3 hash-chain logic, constants
├── server/          Axum HTTP server
│   ├── routes/      /init and /hb handlers
│   ├── session.rs   Session lifecycle: create, verify, advance chain
│   ├── crypto.rs    Ed25519 signature verification (BTreeMap canonical JSON)
│   ├── trust.rs     Behavioral signal validation (mouse speed, pauses)
│   ├── fingerprint  Browser fingerprint sanity checks
│   ├── vm.rs        Random opcode program generator
│   ├── ratelimit.rs Token-bucket rate limiter with periodic eviction
│   └── cleanup.rs   Background task: expire sessions + evict rate limiter
├── wasm/            Rust → WASM client module
│   ├── crypto.rs    Ed25519 keypair, signing, hash computation
│   └── vm.rs        Stack machine executor (PUSH/ADD/SUB/MUL/XOR/AND/OR/ROT/NOT/HASH)
└── frontend/        Vanilla JS glue
    ├── heartbeat.js Session init, heartbeat scheduling, chain advancement
    └── entropy.js   Mouse event collection
```

### Stack Machine

The server generates a random program (8–16 opcodes) on session init. The client executes it on every heartbeat and includes the resulting stack state in the signed payload. This makes each heartbeat structurally unique without requiring any server round-trip.

| Opcode | Mnemonic | Effect |
|--------|----------|--------|
| `0x00` | PUSH u32 | Push 4-byte little-endian literal |
| `0x01` | ADD | Pop 2, push `a + b` (wrapping) |
| `0x02` | SUB | Pop 2, push `a - b` (wrapping) |
| `0x03` | MUL | Pop 2, push `a * b` (wrapping) |
| `0x04` | XOR | Pop 2, push `a ^ b` |
| `0x05` | AND | Pop 2, push `a & b` |
| `0x06` | OR  | Pop 2, push `a \| b` |
| `0x07` | ROT | Pop 2, push `a.rotate_left(b % 32)` |
| `0x08` | NOT | Pop 1, push `!a` (unary) |
| `0x09` | HASH | Blake3 of entire stack → single u32 |

### Hash Chain

```
H(0) = Blake3( session_id ║ pub_key ║ salt₀ )

H(n) = Blake3( saltₙ₋₁ ║ H(n-1) ║ timestamp ║ Blake3(entropy_json) ║ Blake3(stack_json) )
```

Each heartbeat must present `H(n-1)` matching what the server stored. Forging a valid `H(n)` requires knowing the private key (for the signature), the salt (server-side only), and all prior state.

### Signature Canonical Form

The client signs a JSON object with keys sorted alphabetically (matching `JSON.stringify(obj, Object.keys(obj).sort())`):

```json
{
  "entropyData":  { "events": [ { "x": …, "y": …, "t": … } ] },
  "fingerprint":  { "aspectRatio": "…", "devicePixelRatio": "…", "hardwareConcurrency": … },
  "prevHash":     "hex…",
  "sessionId":    "hex…",
  "stackState":   { "stack": […], "ip": … },
  "timestamp":    1234567890123
}
```

The server reconstructs this using `BTreeMap` (alphabetical key order) before calling `VerifyingKey::verify_strict`.

---

## SQLite Schema

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

Sessions are stored in an in-memory SQLite database. All session state is lost on server restart by design — clients re-initialise transparently.

---

## Build

### Prerequisites

- Rust stable (≥ 1.87)
- [`wasm-pack`](https://rustwasm.github.io/wasm-pack/installer/)

### WASM

```bash
wasm-pack build wasm --target web --release
mv wasm/pkg frontend/pkg
```

### Server

```bash
cargo build -p server --release
```

### Dev (all-in-one)

```bash
bash scripts/dev.sh
```

The server serves the `frontend/` directory statically at `/` and the API at `/init` and `/hb`.

---

## Deployment

### Native + systemd

```bash
cargo build -p server --release
sudo cp target/release/server /usr/local/bin/chronoseal
sudo cp chronoseal.service /etc/systemd/system/
sudo systemctl daemon-reload
sudo systemctl enable --now chronoseal
```

### Docker

```bash
docker compose up -d --build
```

### Reverse Proxy

Place ChronoSeal behind nginx, Nginx Proxy Manager, or HAProxy. Enable:

- TLS 1.3
- HTTP/2
- Aggressive upstream timeouts (the heartbeat interval is 12–25s)

---

## Integration

Drop two lines into any protected page:

```html
<script type="module" src="/pkg/antibot_wasm.js"></script>
<script type="module" src="/main.js"></script>
```

`main.js` calls `initHeartbeat()` which handles WASM loading, session init, and schedules all subsequent heartbeats automatically. There is no visible UI, no CAPTCHA, no user interaction required.

---

## Configuration

All tunable constants are in `shared/src/constants.rs`:

| Constant | Default | Description |
|---|---|---|
| `SESSION_ID_LEN` | 32 bytes | Session ID entropy |
| `SALT_LEN` | 16 bytes | Per-heartbeat salt size |
| `HEARTBEAT_MIN_INTERVAL_MS` | 12 000 ms | Minimum heartbeat interval |
| `HEARTBEAT_MAX_INTERVAL_MS` | 25 000 ms | Maximum heartbeat interval (uniform jitter) |
| `EXPIRATION_MINUTES` | 30 min | Session lifetime after last heartbeat |
| `RATE_LIMIT_COUNT` | 5 | Max heartbeats per window |
| `RATE_LIMIT_WINDOW_SECS` | 10 s | Rate limit window |
| `MAX_TIMESTAMP_DRIFT_MS` | 30 000 ms | Anti-replay timestamp window |
| `MIN_MOUSE_TOTAL_DIST` | 10.0 px | Minimum cumulative mouse travel |
| `MAX_MOUSE_AVG_SPEED` | 2.0 px/ms | Maximum average mouse speed |
| `MIN_PAUSE_COUNT` | 1 | Minimum mouse pause events |

---

## License

[GPL-3.0](LICENSE.md)
