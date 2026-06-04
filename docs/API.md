# ChronoSeal API Reference

ChronoSeal exposes a small HTTP API for browser attestation, heartbeat verification, health checks, metrics, and runtime statistics.

This document describes the wire format and acceptance semantics. The internal state model is covered in [ARCHITECTURE.md](ARCHITECTURE.md).

## Base URL

All paths are relative to the ChronoSeal server root.

- Development default: `http://127.0.0.1:3000`
- Production: the HTTPS origin or reverse-proxy path used by the protected site

Production deployments should use HTTPS. The daemon itself can run behind a local reverse proxy.

## Content Type

JSON endpoints expect:

```http
Content-Type: application/json
```

Responses are JSON except `/metrics`, which returns Prometheus text format.

## Endpoint Summary

| Method | Path | Purpose |
|---|---|---|
| `POST` | `/init` | Create a browser attestation session |
| `POST` | `/hb` | Submit and verify a signed heartbeat |
| `GET` | `/health` | Return daemon health |
| `GET` | `/stats` | Return storage/session statistics |
| `GET` | `/metrics` | Return Prometheus-compatible metrics |
| `GET` | `/` | Serve static frontend assets from `frontend_dir` |

## Data Types

Common encodings:

| Value | Encoding |
|---|---|
| Ed25519 public key | 32 raw bytes encoded as 64 hex characters |
| Ed25519 signature | 64 raw bytes encoded as 128 hex characters |
| `session_id` | 32 random bytes encoded as 64 hex characters |
| `salt` | 16 random bytes encoded as 32 hex characters |
| `initial_hash`, `prev_hash`, `gene_commitment` | 32-byte digest encoded as 64 hex characters |
| `opcodes_b64`, `mutation_order_b64` | standard base64 |
| timestamps | Unix time in milliseconds unless otherwise stated |

## `POST /init`

Creates a new attestation session.

### Request

```http
POST /init
Content-Type: application/json
```

```json
{
  "public_key": "hex-encoded 32-byte Ed25519 verifying key"
}
```

| Field | Type | Required | Description |
|---|---|---:|---|
| `public_key` | string | yes | Browser-generated Ed25519 public key as 64 hex characters |

The private key is generated and retained by the browser WASM runtime. It is not sent to the server.

### Successful Response

```http
HTTP/1.1 200 OK
Content-Type: application/json
```

```json
{
  "session_id": "64-char hex string",
  "salt": "32-char hex string",
  "opcodes_b64": "base64-encoded VM program",
  "initial_hash": "64-char hex string",
  "expires_at": 1234567890123,
  "heartbeat_min_interval_ms": 12000,
  "heartbeat_max_interval_ms": 25000,
  "gene_size": 512,
  "mutation_step": 1,
  "mutation_order_b64": "base64-encoded mutation program"
}
```

| Field | Type | Description |
|---|---|---|
| `session_id` | string | Opaque session identifier |
| `salt` | string | Current server salt for the first heartbeat hash computation |
| `opcodes_b64` | string | Randomized VM program executed by the browser runtime |
| `initial_hash` | string | Initial chain head used as `prev_hash` for the first heartbeat |
| `expires_at` | number | Session expiration timestamp in milliseconds |
| `heartbeat_min_interval_ms` | number | Minimum heartbeat delay recommended by the server |
| `heartbeat_max_interval_ms` | number | Maximum heartbeat delay recommended by the server |
| `gene_size` | number | Initial synthetic gene buffer size |
| `mutation_step` | number | Mutation step expected on the first heartbeat |
| `mutation_order_b64` | string | Server-authored mutation order for the first heartbeat |

### Error Behavior

`/init` uses normal route-level error handling for invalid payloads or server failures. Invalid public key length, invalid configured gene size, or storage failure can prevent session creation.

Unlike `/hb`, initialization failures are not part of the silent heartbeat rejection model.

## `POST /hb`

Submits one heartbeat for an existing session.

### Request

```http
POST /hb
Content-Type: application/json
```

```json
{
  "session_id": "64-char hex",
  "prev_hash": "64-char hex",
  "timestamp": 1234567890123,
  "entropy_data": {
    "events": [
      { "x": 412.0, "y": 308.5, "t": 1234.567 }
    ]
  },
  "stack_state": {
    "stack": [2971406957, 1234567890],
    "ip": 42
  },
  "fingerprint": {
    "aspectRatio": "1.7777777778",
    "devicePixelRatio": "2",
    "hardwareConcurrency": 8
  },
  "mutation_step": 1,
  "gene_commitment": "64-char hex",
  "signature": "128-char hex"
}
```

| Field | Type | Required | Description |
|---|---|---:|---|
| `session_id` | string | yes | Session ID from `/init` |
| `prev_hash` | string | yes | Current browser view of the accepted hash-chain head |
| `timestamp` | number | yes | Browser wall-clock timestamp in milliseconds |
| `entropy_data.events` | array | yes | Mouse samples since the previous heartbeat |
| `entropy_data.events[].x` | number | yes | Mouse x coordinate |
| `entropy_data.events[].y` | number | yes | Mouse y coordinate |
| `entropy_data.events[].t` | number | yes | Event timestamp in milliseconds relative to the browser sampling window |
| `stack_state.stack` | array | yes | VM stack output as unsigned 32-bit values |
| `stack_state.ip` | number | yes | VM instruction pointer as an unsigned 16-bit value |
| `fingerprint.aspectRatio` | string | yes | Screen aspect ratio; server accepts numeric strings in range `0.5..=3.0` |
| `fingerprint.devicePixelRatio` | string | yes | Device pixel ratio; server accepts numeric strings in range `(0, 5]` |
| `fingerprint.hardwareConcurrency` | number | yes | Hardware concurrency value; server accepts integers in range `1..=256` |
| `mutation_step` | number | yes | Mutation step currently expected by the server |
| `gene_commitment` | string | yes | Context-bound commitment produced by the WASM mutation preview |
| `signature` | string | yes | Ed25519 signature over the canonical payload |

### Canonical Signing Payload

The signature covers a canonical JSON object with sorted top-level keys:

```json
{
  "entropyData": { "events": [{ "t": 1234.567, "x": 412.0, "y": 308.5 }] },
  "fingerprint": {
    "aspectRatio": "1.7777777778",
    "devicePixelRatio": "2",
    "hardwareConcurrency": 8
  },
  "geneCommitment": "64-char hex",
  "mutationStep": 1,
  "prevHash": "64-char hex",
  "sessionId": "64-char hex",
  "stackState": { "ip": 42, "stack": [2971406957, 1234567890] },
  "timestamp": 1234567890123
}
```

Important details:

- The transport payload uses snake_case for several fields.
- The signed payload uses camelCase names.
- Top-level keys must be serialized deterministically in lexical order.
- The `signature` field is not part of the signed payload.
- Nested serialization must match the server's `serde_json` representation.

The server reconstructs the canonical message from the received request before verifying the Ed25519 signature.

### Accepted Response

```http
HTTP/1.1 200 OK
Content-Type: application/json
```

```json
{
  "status": "ok",
  "next_salt": "32-char hex string",
  "next_mutation_step": 2,
  "next_mutation_order_b64": "base64-encoded mutation program"
}
```

| Field | Type | Description |
|---|---|---|
| `status` | string | Always `ok` |
| `next_salt` | string | Server salt for the next heartbeat |
| `next_mutation_step` | number | Mutation step expected on the next heartbeat |
| `next_mutation_order_b64` | string | Server-authored mutation order for the next heartbeat |

Clients should treat the heartbeat as accepted only when all next-state fields are present.

### Rejected Response

```http
HTTP/1.1 200 OK
Content-Type: application/json
```

```json
{
  "status": "ok"
}
```

Rejected heartbeats omit:

- `next_salt`
- `next_mutation_step`
- `next_mutation_order_b64`

This response shape is intentional. The server does not reveal which validation stage failed.

### Heartbeat Validation Order

The server currently validates heartbeats in this order:

1. Rate-limit check in the route handler.
2. Load session by `session_id`.
3. Check session expiration.
4. Verify Ed25519 signature.
5. Compare `prev_hash` with stored `last_hash`.
6. Compare `mutation_step` with stored `pending_mutation_step`.
7. Apply stored pending mutation to a cloned gene state.
8. Compare expected and submitted `gene_commitment`.
9. Enforce timestamp drift.
10. Validate mouse entropy.
11. Validate fingerprint fields.
12. Compute next hash-chain head.
13. Generate next mutation order and salt.
14. Persist advanced session state.

Any failure after route-level JSON decoding returns the silent rejection body.

## `GET /health`

Returns a basic health response.

```http
GET /health
```

```json
{
  "status": "healthy"
}
```

## `GET /stats`

Returns storage-derived session statistics.

```http
GET /stats
```

```json
{
  "sessions": 1,
  "expired_sessions": 0,
  "max_chain_length": 4
}
```

| Field | Type | Description |
|---|---|---|
| `sessions` | number | Stored session count |
| `expired_sessions` | number | Expired sessions not yet purged |
| `max_chain_length` | number | Highest stored heartbeat chain length |

## `GET /metrics`

Returns Prometheus-compatible text.

```http
GET /metrics
```

```text
# HELP chronoseal_sessions Active ChronoSeal sessions
# TYPE chronoseal_sessions gauge
chronoseal_sessions 1
# HELP chronoseal_expired_sessions Expired sessions not yet removed
# TYPE chronoseal_expired_sessions gauge
chronoseal_expired_sessions 0
# HELP chronoseal_max_chain_length Maximum heartbeat chain length
# TYPE chronoseal_max_chain_length gauge
chronoseal_max_chain_length 4
```

## Client State Rules

After `/init`, the client stores:

- `session_id`
- `initial_hash` as the first `prev_hash`
- current `salt`
- VM opcode program
- committed gene state
- pending mutation step
- pending mutation order

On accepted `/hb`:

1. Commit the local gene preview.
2. Compute the next local hash using the old salt that was active when the heartbeat was sent.
3. Replace current salt with `next_salt`.
4. Replace pending mutation step and order with server-provided values.

On rejected `/hb`:

1. Discard the local gene preview.
2. Do not advance hash-chain state.
3. Do not advance mutation state.
4. Treat the session as suspect or restart attestation.

## WASM Runtime Exports

The generated `chronoseal_wasm` package exposes:

| Function | Signature | Description |
|---|---|---|
| `generate_keypair()` | `() -> string` | Generate an Ed25519 keypair and return public key hex |
| `get_public_key()` | `() -> string` | Return current public key hex, or `""` if no keypair exists |
| `sign_message(msg)` | `(string) -> string` | Sign a UTF-8 payload and return hex signature, or `""` on failure |
| `compute_next_hash(prev, ts, entropy, stack, salt)` | `(string, u64, string, string, string) -> string` | Compute next Blake3 chain hash |
| `run_program(b64)` | `(string) -> JsValue` | Execute a base64 VM program and return stack state |
| `init_gene_state(gene_size)` | `(u32) -> bool` | Initialize the browser gene state |
| `preview_gene_commitment(order_b64, session_id, mutation_step, rounds)` | `(string, string, u64, u8) -> string` | Preview next mutation commitment |
| `commit_gene_preview()` | `() -> bool` | Commit the preview after accepted heartbeat |
| `discard_gene_preview()` | `() -> void` | Discard preview after rejection or error |
| `current_gene_commitment(session_id, mutation_step)` | `(string, u64) -> string` | Return current committed gene commitment |

String-returning functions use `""` to signal failure. Callers must handle empty strings explicitly.
