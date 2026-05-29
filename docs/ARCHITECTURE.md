# ChronoSeal Architecture

ChronoSeal is a Unix-native cryptographic attestation daemon that validates browser session continuity through deterministic VM execution, chained cryptographic state, and a shared Synthetic Gene Mutation Engine.

## Overview

ChronoSeal is designed as a production-grade infrastructure component, not as a consumer-facing widget. It is a lightweight daemon that can be operated, monitored, and integrated like any other native Linux service.

Key characteristics:

* Unix-native daemon with systemd-compatible lifecycle
* CLI-first control plane and configuration
* Shared Rust/WASM runtime for server/client parity
* Modular storage backend abstraction (`sqlite-in-memory`, `sqlite-disk`, `valkey`)
* Silent rejection semantics for attacker resilience
* Privacy-preserving ephemeral session state

## Core Components

### `shared/`

Shared protocol and runtime primitives used by both the server and the browser runtime:

* Cryptographic primitives: Blake3, Ed25519
* Hash chain logic and session commitment handling
* Synthetic gene model and deterministic mutation engine
* Serialization, encoding, and canonical signing helpers

### `server/`

The server crate implements the runtime daemon:

* `routes/init.rs` — session initialization API
* `routes/heartbeat.rs` — heartbeat verification API
* `session.rs` — session lifecycle, mutation parity, and heartbeat validation
* `storage.rs` — backend abstraction and persistence
* `crypto.rs` — signature verification and key handling
* `trust.rs` — behavioral entropy and sanity validation
* `ratelimit.rs` — per-session request throttling
* `cleanup.rs` — expiration and eviction tasks
* `runtime.rs` — daemon bootstrap, metrics, and state management

### `wasm/`

The client runtime crate compiles to WebAssembly and powers attestation in the browser.

* `crypto.rs` — in-WASM signing and hash computation
* `vm.rs` — randomized opcode VM execution
* `vm_extensions.rs` — synthetic gene mutation preview and commit lifecycle

### `frontend/`

Static browser integration code that loads the WASM module, orchestrates init/heartbeat flow, and collects browser entropy.

## v0.6.0 Innovation

The primary innovation in v0.6.0 is the **Synthetic Gene Mutation Engine**.

This layer adds a deterministic, shared server/WASM mutation handshake to the existing heartbeat continuity model.

Key v0.6.0 behavior:

* `mutation_order_b64` is issued at session initialization and after every accepted heartbeat
* `mutation_step` is tracked on both client and server
* `gene_commitment` is computed locally in WASM and validated by the server
* mutation state is persisted per session and advanced only on accepted heartbeats
* scalar mutation programs are deterministic and bounded in cost

This makes replay and tampering attacks significantly more expensive while preserving the existing privacy-first and silent-failure semantics.

## Architecture Diagram

```
Browser                                Server
  ┌──────────────────────────────────────────────────────────────┐
  │ frontend/ + WASM runtime                                      │
  │  - generate_keypair()                                         │
  │  - sign_message()                                             │
  │  - compute_next_hash()                                        │
  │  - run_program()                                              │
  │  - preview_gene_commitment()                                  │
  │  - commit_gene_preview()                                      │
  │                                                              │
  │  POST /init ->                                              │
  │  POST /hb   ->                                              │
  └──────────────────────────────────────────────────────────────┘
                       │
                       ▼
  ┌──────────────────────────────────────────────────────────────┐
  │ server/                                                      │
  │  - signature validation                                       │
  │  - hash chain continuity                                      │
  │  - rate limiting                                              │
  │  - behavioral trust checks                                    │
  │  - mutation step validation                                   │
  │  - gene commitment verification                               │
  │  - session persistence                                        │
  │  - metrics and health                                          │
  └──────────────────────────────────────────────────────────────┘
                       │
                       ▼
  ┌──────────────────────────────────────────────────────────────┐
  │ storage backends                                              │
  │  - sqlite-in-memory                                            │
  │  - sqlite-disk                                                 │
  │  - valkey compatibility mode                                   │
  └──────────────────────────────────────────────────────────────┘
```

## Storage Backends

ChronoSeal supports pluggable backend modes using the `db_type` configuration option.

* `sqlite-in-memory` — default ephemeral session storage. No persistence across restarts.
* `sqlite-disk` — persisted SQLite database on disk through `db_path`.
* `valkey` — compatibility mode for alternative storage backends, currently supported alongside SQLite compatibility semantics.

## Runtime Philosophy

ChronoSeal is intentionally designed to behave like traditional Unix infrastructure software:

* explicit CLI operations (`run`, `status`, `health`, `config`, `metrics`, `stats`, `db-type`)
* structured logging for `journalctl`
* PID file management and graceful shutdown
* systemd sandbox support
* runtime configuration via TOML and CLI overrides
* clear separation of protocol, persistence, and runtime concerns

## Integration Points

* Browser clients consume the WASM module and call `/init` and `/hb`
* Existing sites can proxy these API routes through their own web server
* Frontend assets can be served by ChronoSeal directly or mounted in a sidecar deployment
* TLS termination should be handled by a reverse proxy in production

## Operating Assumptions

ChronoSeal is not a general-purpose authentication service. It is a cryptographic attestation and anti-automation layer intended to be integrated with existing site logic.

It assumes:

* browser clients can execute WASM
* heartbeats will arrive every 12–25 seconds
* session state can be safely persisted in SQLite or Valkey
* service operators want Unix-native systemd deployment and observability
