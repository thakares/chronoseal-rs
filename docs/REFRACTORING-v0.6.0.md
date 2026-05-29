# ChronoSeal v0.6.0 — Refactoring and System Upgrade

ChronoSeal v0.6.0 is a major architecture and protocol update that transforms the project from a lightweight heartbeat service into a mature Unix-native attestation daemon with deterministic mutation parity and pluggable storage backends.

## Summary of Changes

* Introduced the **Synthetic Gene Mutation Engine** for deterministic mutation parity across server and WASM.
* Added server-side validation of `mutation_step` and `gene_commitment`.
* Centralized shared protocol logic in `shared/` for server/WASM parity.
* Added support for multiple storage backend modes: `sqlite-in-memory`, `sqlite-disk`, and `valkey` compatibility.
* Hardened runtime architecture with `systemd` readiness, graceful shutdown, PID file support, and structured logging.
* Expanded CLI with rich subcommands and effective runtime configuration.
* Preserved silent rejection semantics while improving anti-replay and liveness guarantees.

## Why This Refactor?

The previous model relied on heartbeat continuity and behavioral entropy alone. v0.6.0 strengthens the protocol by adding a second, deterministic state progression channel:

* each heartbeat now includes a mutation step and commitment
* the server authoritatively selects the next mutation program
* the client must preview and commit the same state locally in WASM
* the server rejects any mismatch silently

This raises the cost of developing a successful automation attack because the attacker must now maintain both a valid chain and a valid mutation progression state.

## Core Architecture Changes

### Shared Protocol Code

`shared/` now contains:

* gene model and commitment hashing
* mutation opcode semantics
* request/response payload structures
* canonical signing support
* VM execution logic shared by server and WASM

Moving mutation semantics into `shared/` eliminates subtle server/client divergence bugs and enables deterministic cross-runtime testing.

### Mutation Handshake

v0.6.0 adds the following data to the protocol:

* `mutation_step`
* `mutation_order_b64`
* `gene_commitment`
* `next_mutation_step`
* `next_mutation_order_b64`

These fields are now part of the session initialization and heartbeat exchange.

### Server Session State

The session schema now stores:

* committed gene bytes
* committed environment records
* pending mutation order
* pending mutation step

The server advances this state only after a heartbeat is accepted.

### Deterministic WASM Preview

The WASM runtime exposes:

* `init_gene_state()`
* `preview_gene_commitment()`
* `commit_gene_preview()`
* `discard_gene_preview()`
* `current_gene_commitment()`

This makes the client-side mutation lifecycle explicit and deterministic.

### Backend Abstraction

The server runtime now supports a configurable `db_type`.

* `sqlite-in-memory` — default runtime storage with ephemeral session semantics
* `sqlite-disk` — persistent SQLite storage for stateful deployments
* `valkey` — compatibility mode for alternative storage backends

This abstraction makes ChronoSeal easier to operate in both stateless and stateful environments.

### CLI and Service Integration

v0.6.0 improves the CLI surface with operational commands and service introspection.

* `chronoseal run`
* `chronoseal status`
* `chronoseal health`
* `chronoseal config`
* `chronoseal metrics`
* `chronoseal stats`
* `chronoseal db-type`
* `chronoseal completion`
* `chronoseal version`

The runtime now includes PID file handling and graceful termination.

## Testing and Validation

The refactor includes extensive tests for:

* server/WASM parity across mutation sequences
* malformed mutation payload rejection
* replay attack rejection
* mutation step mismatch rejection
* stateful session update semantics
* runtime database mode validation

The codebase now supports deterministic table-driven tests and fuzz-style random program validation.

## Operational Impact

This release makes ChronoSeal suitable for production deployment in Linux environments and for integration into existing web application stacks.

The combination of deterministic mutation parity and shared protocol implementation improves both security and maintainability.
