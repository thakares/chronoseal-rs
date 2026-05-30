# ChronoSeal Architecture

ChronoSeal is a Unix-native browser attestation daemon. It validates browser session continuity by combining signed heartbeats, Blake3 hash-chain progression, deterministic VM execution, behavioral sanity checks, and a shared Synthetic Gene Mutation Engine that runs on both the server and the browser WASM runtime.

This document describes the system architecture, state model, validation pipeline, trust boundaries, and operational assumptions. The API wire format is documented separately in [API.md](API.md), and deployment guidance is documented in [DEPLOYMENT.md](DEPLOYMENT.md).

## Architectural Goals

ChronoSeal is designed as infrastructure software rather than a consumer-facing widget. The main goals are:

- Keep the server small, inspectable, and operable as a normal Unix daemon.
- Use deterministic client/server computation so the server can verify browser-side progression without trusting browser claims blindly.
- Make replay, stale state reuse, and incomplete automation expensive.
- Preserve privacy by using short-lived session state instead of persistent identity tracking.
- Avoid attacker feedback oracles by returning indistinguishable success-shaped responses for rejected heartbeats.
- Keep browser integration lightweight: static JavaScript plus a Rust-generated WASM package.

ChronoSeal does not attempt to prove that a human is present. It attempts to prove that a client is maintaining the expected live browser-side cryptographic and mutation state.

## System Context

```text
Protected browser origin
        |
        | static files and API calls
        v
+------------------------------+
| Browser                      |
| - frontend JavaScript        |
| - chronoseal_wasm runtime    |
| - Ed25519 session key        |
| - VM and gene state          |
+---------------+--------------+
                |
                | POST /init
                | POST /hb
                v
+------------------------------+
| ChronoSeal daemon            |
| - Axum HTTP routes           |
| - session verifier           |
| - storage abstraction        |
| - metrics and health         |
+---------------+--------------+
                |
                | SessionRecord
                v
+------------------------------+
| Storage backend              |
| - sqlite-in-memory           |
| - sqlite-in-disk             |
| - valkey                     |
+------------------------------+
```

ChronoSeal can serve the frontend files itself or sit behind a reverse proxy. TLS termination should happen before traffic reaches the daemon in production.

## Workspace Components

The repository is a Rust workspace with three runtime crates and one static frontend directory.

### `shared/`

`shared/` contains protocol and deterministic runtime code used by both the server and WASM crates.

Responsibilities:

- wire protocol structs for `/init` and `/hb`
- Blake3 hash-chain helpers
- synthetic gene state representation
- environment encoding and validation
- mutation program generation, encoding, decoding, and execution
- deterministic VM extension opcode semantics

Important files:

| File | Responsibility |
|---|---|
| `protocol.rs` | `InitRequest`, `InitResponse`, `HeartbeatRequest`, `HeartbeatResponse`, and supporting payload types |
| `hashing.rs` | initial and next hash-chain computation |
| `gene.rs` | gene state, environment records, validation, and context-bound commitment |
| `vm_extensions.rs` | mutation order generation, opcode interpreter, execution tracing, and tests |
| `constants.rs` | protocol and execution bounds |

`shared/` is the determinism boundary. Any logic that must agree between server and browser belongs here rather than in server-only or frontend-only code.

### `server/`

`server/` builds the `chronoseal` binary. It owns daemon lifecycle, HTTP routing, session verification, storage, metrics, configuration, and CLI behavior.

Important files:

| File | Responsibility |
|---|---|
| `main.rs` | CLI command dispatch |
| `cli.rs` | command, flag, and environment variable definitions |
| `config.rs` | defaults, TOML loading, environment overrides, validation |
| `runtime.rs` | daemon startup, Axum router, health, metrics, stats, graceful shutdown |
| `routes/init.rs` | `POST /init` handler |
| `routes/heartbeat.rs` | `POST /hb` handler and silent rejection response shape |
| `session.rs` | session creation, heartbeat verification, state advancement |
| `crypto.rs` | canonical signing payload and Ed25519 signature verification |
| `storage.rs` | `DbPool`, SQLite, Valkey compatibility, session persistence, stats |
| `trust.rs` | mouse entropy validation |
| `fingerprint.rs` | browser signal validation |
| `ratelimit.rs` | per-session rate limiting |
| `cleanup.rs` | expired session removal |

The server treats the browser as untrusted. Browser-supplied values are accepted only after signature, continuity, timing, behavioral, and mutation checks pass.

### `wasm/`

`wasm/` compiles to the browser runtime package with `wasm-pack --target web`.

Responsibilities:

- generate and hold the browser-local Ed25519 keypair
- sign canonical heartbeat payloads
- compute hash-chain values used by the browser integration
- execute randomized VM programs
- maintain committed and preview synthetic gene state
- preview mutation commitments before a heartbeat is submitted
- commit or discard preview state after server response

Important files:

| File | Responsibility |
|---|---|
| `crypto.rs` | key generation, public key export, message signing |
| `vm.rs` | base VM program execution |
| `vm_extensions.rs` | gene initialization, mutation preview, commit, discard, current commitment |

The WASM runtime is not a trusted execution environment. It is useful because it forces a browser client to implement the same state transitions as the server and makes simple HTTP automation insufficient.

### `frontend/`

`frontend/` contains static JavaScript and browser assets. It loads `frontend/pkg/chronoseal_wasm.js`, calls `/init`, periodically sends `/hb`, and coordinates browser-side state transitions.

The frontend is intentionally thin. Durable protocol rules live in Rust, not in handwritten JavaScript.

## Runtime Topology

The daemon builds a single Axum application with:

| Route | Method | Purpose |
|---|---|---|
| `/init` | `POST` | create a new attestation session |
| `/hb` | `POST` | verify and advance a heartbeat |
| `/health` | `GET` | health probe |
| `/metrics` | `GET` | Prometheus-compatible metrics |
| `/stats` | `GET` | storage/session statistics |
| `/` | `GET` | static frontend assets from `frontend_dir` |

Shared runtime state is held in `AppState`:

- `db_pool`: storage backend handle
- `rate_limiter`: process-local rate limiter
- `config`: runtime configuration snapshot behind an `RwLock`

Configuration is resolved in this order:

1. CLI flags
2. `CHRONOSEAL_*` environment variables
3. TOML configuration file
4. built-in defaults

## Session State Model

The server persists one `SessionRecord` per active session.

| Field | Meaning |
|---|---|
| `session_id` | random 32-byte session identifier encoded as hex |
| `public_key` | browser-generated Ed25519 verifying key |
| `salt` | current server salt for hash-chain progression |
| `last_hash` | current accepted hash-chain head |
| `chain_length` | number of accepted chain states including initialization |
| `created_at` | creation timestamp in milliseconds |
| `last_seen` | timestamp of last accepted heartbeat |
| `expires_at` | session expiration timestamp in milliseconds |
| `gene` | committed synthetic gene byte buffer |
| `environment` | encoded environment records |
| `pending_mutation` | server-issued mutation program for the next heartbeat |
| `pending_mutation_step` | mutation step expected on the next heartbeat |

The committed server state advances only after a heartbeat passes all validation checks. Failed heartbeats do not update `last_hash`, `salt`, `gene`, `environment`, `pending_mutation`, or `pending_mutation_step`.

## Initialization Flow

```text
Browser/WASM                         Server
------------                         ------
generate_keypair()
public key
      |
      | POST /init { public_key }
      v
                                  validate public key length
                                  create GeneState
                                  generate session_id
                                  generate salt
                                  compute initial_hash
                                  generate VM opcodes
                                  generate mutation step 1
                                  persist SessionRecord
      ^
      | InitResponse
      |
store session_id, salt,
initial_hash, opcodes,
gene_size, mutation order
```

Initialization creates the first server-side commitment state but does not prove liveness. Liveness begins with accepted heartbeats.

The initial response contains:

- `session_id`
- `salt`
- `opcodes_b64`
- `initial_hash`
- `expires_at`
- heartbeat interval bounds
- `gene_size`
- `mutation_step`
- `mutation_order_b64`

## Heartbeat Flow

```text
Browser/WASM                                      Server
------------                                      ------
execute VM program
collect entropy and fingerprint data
preview pending gene mutation
build canonical signing payload
sign with Ed25519 private key
      |
      | POST /hb HeartbeatRequest
      v
                                                 load session
                                                 check expiration
                                                 verify signature
                                                 check hash continuity
                                                 check mutation step
                                                 apply pending mutation
                                                 compare gene commitment
                                                 check timestamp drift
                                                 validate mouse entropy
                                                 validate fingerprint
                                                 compute next hash
                                                 generate next mutation
                                                 generate next salt
                                                 persist advanced state
      ^
      | accepted: status + next salt + next mutation
      | rejected: { "status": "ok" }
      |
commit preview on accepted response
discard or stop on rejected response
```

Accepted heartbeats return `next_salt`, `next_mutation_step`, and `next_mutation_order_b64`.

Rejected heartbeats return only:

```json
{
  "status": "ok"
}
```

This silent rejection behavior is part of the security model. It prevents the API from acting as an oracle for signature, timing, mutation, or behavior failures.

## Verification Pipeline

Heartbeat verification occurs in `server/src/session.rs`.

The current validation order is:

1. Load the session by `session_id`.
2. Reject if the session is missing.
3. Reject if `now > expires_at`.
4. Verify the Ed25519 signature over the canonical payload.
5. Decode and compare `prev_hash` with the stored `last_hash`.
6. Compare request `mutation_step` with stored `pending_mutation_step`.
7. Decode the stored gene environment.
8. Apply the stored `pending_mutation` to a cloned server gene state.
9. Compute the expected `gene_commitment` with session and step context.
10. Compare the request `gene_commitment` with the expected commitment.
11. Enforce timestamp drift bounds.
12. Validate mouse entropy.
13. Validate browser fingerprint fields.
14. Compute the next hash-chain value.
15. Generate the next mutation order.
16. Generate the next salt.
17. Persist the advanced session state.

The verifier performs state mutation only after validation succeeds. This preserves replay resistance and avoids desynchronizing the server after invalid requests.

## Canonical Signing Boundary

The heartbeat signature covers a canonical JSON payload built from:

- `entropyData`
- `fingerprint`
- `geneCommitment`
- `mutationStep`
- `prevHash`
- `sessionId`
- `stackState`
- `timestamp`

The server constructs this payload using a `BTreeMap`, which orders top-level keys deterministically before serializing. The transport request uses snake_case field names, while the signed payload uses camelCase names that match the browser-side canonical message.

The signature does not cover the `signature` field itself.

## Hash-Chain Boundary

Each accepted heartbeat advances a Blake3 hash chain.

Inputs include:

- previous hash-chain head
- heartbeat timestamp
- entropy data
- VM stack state
- current server salt

The server stores only the current accepted head as `last_hash`. A replayed heartbeat with an old `prev_hash` fails because the stored `last_hash` has already advanced.

The salt rotates after every accepted heartbeat. The next salt is returned only on acceptance, so rejected clients do not receive the material needed for the next valid chain step.

## Synthetic Gene Mutation Engine

The Synthetic Gene Mutation Engine provides an additional deterministic continuity check.

Core concepts:

- `GeneState`: committed gene byte buffer plus environment records.
- `MutationOrder`: mutation step plus encoded mutation program.
- `pending_mutation`: the server-authored program expected on the next heartbeat.
- `gene_commitment`: context-bound commitment over the candidate gene state, `session_id`, and `mutation_step`.

The server and WASM runtime both execute the same mutation semantics from `shared/vm_extensions.rs`.

Mutation lifecycle:

1. Server stores a pending mutation program and step.
2. Browser previews that mutation against its committed gene state.
3. Browser sends the resulting `gene_commitment`.
4. Server applies the same mutation to a clone of its committed gene state.
5. Server compares the expected commitment with the browser commitment.
6. On success, server commits the candidate state and issues the next mutation.
7. Browser commits its preview only after receiving an accepted response.

This design prevents a client from advancing mutation state independently of the server. The mutation order is server-authored, step-bound, and accepted only once.

## Behavioral Trust Checks

ChronoSeal includes lightweight behavioral checks. These checks are not a complete human verification system; they are an automation cost signal.

Current checks include:

- minimum mouse activity, when enabled
- minimum total mouse movement distance
- maximum average mouse speed
- minimum pause count
- timestamp drift bound
- basic fingerprint field validation

The checks are intentionally bounded and configurable. They should be treated as one layer in the attestation pipeline, not as the primary security primitive.

## Storage Architecture

Storage is abstracted by `DbPool`.

| Backend | `db_type` | Characteristics |
|---|---|---|
| SQLite memory | `sqlite-in-memory` | default, process-local, ephemeral |
| SQLite disk | `sqlite-in-disk` | persisted SQLite file at `db_path` |
| Valkey | `valkey` | Valkey-compatible session store utilizing thread-safe connection pooling |

The storage layer must support:

- insert session
- load session
- update session
- delete expired sessions
- report statistics

`valkey` mode reads `CHRONOSEAL_VALKEY_ADDR`, defaulting to `127.0.0.1:6666`. It establishes a connection pool using `r2d2` and the `redis` client crate. Session IDs are indexed using native Valkey sets (`sessions:ids`) to minimize overhead and avoid lock contention, while individual sessions are persisted with a native TTL (`SET ... EX`) matching their expiration times. If connection setup fails, it logs a warning and falls back to in-memory SQLite.

## Metrics and Observability

ChronoSeal exposes two operational surfaces:

- CLI commands: `status`, `health`, `metrics`, `stats`, `config check`
- HTTP endpoints: `/health`, `/metrics`, `/stats`

The metrics endpoint reports storage-derived counters including:

- active sessions
- expired sessions
- maximum observed chain length

The daemon uses structured tracing and can log to journald through normal systemd operation. Operators should avoid debug logging in production because internal identifiers may appear in logs.

## Trust Boundaries

### Browser Boundary

The browser is untrusted. It may lie about entropy, fingerprint values, VM output, mutation commitment, timing, and session identifiers.

Mitigation:

- signature verification binds payloads to the browser session key
- hash-chain checks reject stale state
- mutation commitment checks reject incorrect gene progression
- timing and behavioral checks reject implausible requests

### WASM Boundary

WASM code runs in the browser and is therefore not trusted as secure enclave code.

Mitigation:

- the server independently recomputes critical deterministic state
- private key custody raises automation cost but is not treated as hardware-backed secrecy
- failures do not reveal detailed reasons to callers

### Storage Boundary

Storage is trusted for session continuity. If storage is lost, sessions cannot continue. If storage is tampered with, attestation integrity can be affected.

Mitigation:

- use proper filesystem permissions for SQLite disk mode
- deploy Valkey on a trusted network or protected socket
- keep ChronoSeal behind normal host and service hardening

### Network Boundary

ChronoSeal expects production traffic to be protected by TLS. Plaintext deployment weakens confidentiality and makes traffic analysis easier.

Mitigation:

- terminate TLS at a reverse proxy or load balancer
- keep `/init` and `/hb` same-origin with protected content when possible
- avoid exposing internal metrics broadly

## Failure Semantics

ChronoSeal intentionally separates transport success from attestation success.

| Failure class | HTTP behavior | State mutation |
|---|---|---|
| malformed route-level request | normal HTTP error handling | no session advancement |
| invalid heartbeat semantics | `200 OK` with `{"status":"ok"}` | no session advancement |
| rejected heartbeat | `200 OK` with `{"status":"ok"}` | no session advancement |
| accepted heartbeat | `200 OK` with next-state fields | session state advances from the verifier's perspective |

This ambiguity reduces attacker feedback. Application integrations must check for the presence of `next_salt`, `next_mutation_step`, and `next_mutation_order_b64` rather than treating any `status: ok` as an accepted heartbeat.

## Invariants

The architecture relies on these invariants:

- A session has exactly one expected `pending_mutation_step` at a time.
- A pending mutation is consumed only by an accepted heartbeat.
- `last_hash` changes only after a heartbeat passes verification.
- `salt` changes only after a heartbeat passes verification.
- `gene` and `environment` change only after mutation commitment validation succeeds.
- The next mutation order is generated only from an accepted candidate state.
- Rejected heartbeats do not reveal the failed validation stage.
- Browser-side preview state is committed only after an accepted heartbeat response.

Breaking these invariants can introduce replay acceptance, client/server desynchronization, or oracle behavior.

## Concurrency Notes

ChronoSeal currently verifies a heartbeat by loading a session, computing candidate state, and writing the updated record back to storage. The intended operational model is one live heartbeat stream per browser session.

Concurrent heartbeats for the same `session_id` should naturally collapse to at most one accepted progression because both requests present the same `prev_hash` and `mutation_step`; after the first accepted update, the second request becomes stale. Storage backends must preserve update visibility strongly enough for this assumption to hold.

## Deployment Shape

Typical production topology:

```text
Internet
   |
   v
TLS reverse proxy
   |
   v
chronoseal daemon on 127.0.0.1:3000
   |
   v
SQLite disk or Valkey storage
```

Recommended deployment properties:

- run under systemd with a dedicated service user
- bind to localhost behind a reverse proxy unless direct exposure is required
- serve over HTTPS
- keep debug logs disabled
- monitor `/health`, `/metrics`, and `/stats`
- use `sqlite-in-memory` for ephemeral local sessions
- use `sqlite-in-disk` or `valkey` when sessions must survive process restarts

## Limitations

ChronoSeal is not:

- a user authentication system
- a CAPTCHA
- a fraud scoring engine
- a hardware attestation system
- a persistent identity framework
- a complete defense against fully resourced browser farms

It is a protocol layer that makes browser automation and replay more expensive by requiring correct, continuous, stateful execution.

## Related Documents

- [API Reference](API.md)
- [Deployment Guide](DEPLOYMENT.md)
- [Threat Model](THREAT_MODEL.md)
- [WASM Build Guide](WASM_BUILD.md)
- [Design Philosophy](DESIGN-PHILOSOPHY.md)
- [Privacy Policy](PRIVACY%20POLICY.md)
