# ChronoSeal v0.6.0 Refactoring and System Upgrade

ChronoSeal v0.6.0 changed the project from a lightweight heartbeat prototype into a Unix-native attestation daemon with shared server/WASM protocol logic, deterministic mutation parity, operational CLI commands, and pluggable storage modes.

This document summarizes the architectural changes introduced in the v0.6.0 line.

## Summary

Major changes:

- introduced the Synthetic Gene Mutation Engine
- added server-side validation of `mutation_step` and `gene_commitment`
- moved protocol and deterministic mutation logic into `shared/`
- added `chronoseal-wasm` browser runtime support for mutation preview and commit
- expanded persisted session state with gene and pending mutation fields
- added storage modes: `sqlite-in-memory`, `sqlite-in-disk`, and `valkey`
- added health, metrics, stats, config, status, completion, and version CLI surfaces
- added PID file handling, structured logging, and graceful shutdown behavior
- preserved silent heartbeat rejection semantics

## Motivation

The earlier model relied mainly on:

- heartbeat timing
- behavioral entropy
- hash-chain continuity
- signature verification

v0.6.0 added a second deterministic state channel: a server-authored synthetic gene mutation sequence. This makes successful automation maintain both:

- the cryptographic hash/signature chain
- the synthetic mutation state expected by the server

## Shared Crate Refactor

`shared/` now owns the parts of the protocol that must remain identical across server and browser runtime:

- request and response structs
- hashing helpers
- synthetic gene state
- mutation environment encoding
- mutation order generation and encoding
- opcode execution semantics
- protocol constants

This reduces the risk of server/WASM drift.

## Mutation Handshake

New protocol fields:

- `gene_size`
- `mutation_step`
- `mutation_order_b64`
- `gene_commitment`
- `next_mutation_step`
- `next_mutation_order_b64`

Lifecycle:

1. `/init` returns mutation step 1 and a server-authored mutation order.
2. The browser previews the mutation in WASM.
3. The browser signs and submits the resulting `gene_commitment`.
4. The server applies the same pending mutation to its committed state.
5. The server compares commitments.
6. On success, server commits the candidate state and issues the next mutation.
7. The browser commits its preview only after receiving the accepted response.

## Session Schema Changes

The persisted session record now includes:

- committed gene bytes
- encoded environment records
- pending mutation program
- pending mutation step

State advances only after a heartbeat is accepted. Rejected heartbeats do not rotate salt, update hash state, commit gene state, or consume the pending mutation.

## WASM Runtime Changes

The WASM crate now supports:

- `generate_keypair()`
- `get_public_key()`
- `sign_message()`
- `compute_next_hash()`
- `run_program()`
- `init_gene_state()`
- `preview_gene_commitment(order_b64, session_id, mutation_step, rounds)`
- `commit_gene_preview()`
- `discard_gene_preview()`
- `current_gene_commitment(session_id, mutation_step)`

The generated package uses the `chronoseal_wasm` prefix.

## Storage Refactor

The storage layer is abstracted behind `DbPool`.

Supported modes:

| Mode | Behavior |
|---|---|
| `sqlite-in-memory` | default ephemeral in-process SQLite |
| `sqlite-in-disk` | persisted SQLite database at `db_path` |
| `valkey` | Valkey-compatible external store |

The storage interface supports insert, load, update, delete expired sessions, and stats.

## CLI and Runtime Changes

The `chronoseal` binary now provides:

- `run`
- `status`
- `health`
- `config check`
- `generate keypair`
- `version`
- `db-type`
- `metrics`
- `stats`
- `completion`

The daemon exposes:

- `POST /init`
- `POST /hb`
- `GET /health`
- `GET /metrics`
- `GET /stats`
- static frontend serving at `/`

## Validation Improvements

The heartbeat verifier now checks:

- session presence
- expiration
- signature
- hash-chain continuity
- mutation step
- mutation commitment parity
- timestamp drift
- behavioral mouse checks
- fingerprint ranges
- rate limiting at the route layer

Accepted heartbeats return next-state fields. Rejected heartbeats return only `{"status":"ok"}`.

## Testing Impact

The refactor added or strengthened tests for:

- gene environment encoding and validation
- mutation opcode behavior
- mutation order round-trips
- deterministic mutation generation with seeded RNG
- server/client mutation parity
- random program divergence resistance
- replay rejection
- mutation step mismatch rejection
- mutation commitment tamper rejection
- storage backend stats
- route-level silent rejection behavior

## Operational Impact

v0.6.0 makes ChronoSeal more suitable for deployment as a real service:

- explicit daemon lifecycle
- CLI-first operations
- systemd-oriented install path
- health and metrics endpoints
- configurable persistence
- shared protocol implementation
- clearer docs and threat model

## Compatibility Notes

Important names in the current implementation:

- binary: `chronoseal`
- server crate: `chronoseal-server`
- WASM crate: `chronoseal-wasm`
- generated WASM module prefix: `chronoseal_wasm`
- persistent SQLite mode: `sqlite-in-disk`

Older docs or integrations may refer to `sqlite-disk`, `server`, or `antibot_wasm`; those names are stale for the current codebase.
