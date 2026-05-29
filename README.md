# ChronoSeal

<p align="center">
  <img src="logo/chronoseal.svg" width="220" alt="ChronoSeal Logo">
</p>

<p align="center">
  <strong>Unix-native cryptographic attestation daemon for browser session continuity.</strong>
</p>

<p align="center">
  Privacy-first • Deterministic WASM parity • Silent rejection • Low overhead
</p>

<p align="center">
  <a href="https://github.com/thakares/chronoseal-rs/blob/main/LICENSE">
  <img src="https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg" alt="License: MIT OR Apache-2.0">
</a>
  <a href="https://github.com/thakares/chronoseal-rs">
    <img src="https://img.shields.io/badge/rust-stable%20%E2%89%A5%201.87-orange.svg" alt="Rust stable ≥ 1.87">
  </a>
  <a href="https://github.com/thakares/chronoseal-rs/blob/main/docs/REFRACTORING-v0.6.0.md">
    <img src="https://img.shields.io/badge/version-v0.6.0-green.svg" alt="v0.6.0">
  </a>
  <img src="https://img.shields.io/badge/wasm-rust--compiled-blueviolet.svg" alt="WASM">
</p>

---

ChronoSeal is a mature Unix-native cryptographic attestation daemon for browser session continuity and anti-automation defense.

It provides a low-overhead, privacy-respecting proof-of-runtime system built around a deterministic **Synthetic Gene Mutation Engine** and a silent, replay-resistant heartbeat protocol.

v0.6.0 introduces the core innovation: a deterministic synthetic gene mutation chain with server/WASM parity, stronger liveness guarantees, and a domain-separated mutation commitment handshake.

---

## What ChronoSeal Provides

* Native Linux daemon with hardened `systemd` integration
* Deterministic mutation engine running in both server Rust and client WASM
* Silent rejection semantics for attacker resilience
* Multi-backend storage: `sqlite-in-memory`, `sqlite-disk`, and `valkey`
* CLI-first operation with rich subcommands
* Structured logging, PID file management, graceful shutdown
* Prometheus-compatible metrics and runtime statistics
* Lightweight browser runtime with WASM-based attestation
* Privacy-first design with ephemeral session state and no persistent tracking

---

## Why ChronoSeal

ChronoSeal raises the operational cost of automation by combining:

* cryptographic session continuity
* deterministic VM execution
* behavioral entropy validation
* mutation commitment parity
* silent, ambiguous rejection behavior

This is not a fingerprinting or surveillance platform. ChronoSeal is designed to make automation expensive, not to collect user identities.

---

## Quick Start

### Install

```bash
sudo bash scripts/install.sh
```

### Verify status

```bash
chronoseal status --format json
```

### Health probe

```bash
chronoseal health
```

### View metrics

```bash
chronoseal metrics
```

### Follow logs

```bash
sudo journalctl -u chronoseal -f
```

---

## CLI Overview

```bash
chronoseal --help
```

### Available commands

| Command      | Description                                |
| ------------ | ------------------------------------------ |
| `run`        | Run the ChronoSeal daemon                  |
| `status`     | Report daemon status                       |
| `health`     | Perform daemon health probe                |
| `config`     | Validate and print effective configuration |
| `generate`   | Generate operational material              |
| `db-type`    | List database backend support status       |
| `metrics`    | Output Prometheus metrics                  |
| `stats`      | Print runtime statistics                   |
| `completion` | Generate shell completions                 |
| `version`    | Print version/build information            |

---

## Example

```bash
chronoseal status --format json
```

```json
{
  "running": true,
  "healthy": true,
  "bind": "0.0.0.0:3000",
  "pid_file": "/run/chronoseal.pid",
  "pid": 79459
}
```

---

## Architecture Summary

ChronoSeal is composed of three primary runtime components:

* `shared/` — shared cryptographic primitives, hash chaining, gene model, and mutation engine used by both server and WASM
* `server/` — Axum-based Unix-native daemon, session lifecycle, storage, trust evaluation, and `POST /init` / `POST /hb` routes
* `wasm/` — browser runtime for key generation, signature creation, VM execution, and mutation preview/commit lifecycle

### Key innovations in v0.6.0

* Synthetic Gene Mutation Engine with deterministic, shared opcode semantics
* Server-side gene commitment validation on every heartbeat
* `mutation_step` and `mutation_order_b64` handshake in init and heartbeat responses
* `db_type` runtime backend selection with SQLite and Valkey support

---

## How ChronoSeal Works

ChronoSeal establishes continuity by chaining signed heartbeats between client and server.

### Session flow

1. Client loads the WASM runtime and generates an Ed25519 keypair in WASM memory.
2. Client calls `POST /init` with the public key.
3. Server creates an ephemeral session and returns a `session_id`, initial salt, VM program, and mutation order metadata.
4. Client executes the VM program, collects browser entropy, previews the mutation commitment, signs the heartbeat payload, and sends `POST /hb`.
5. Server verifies signature, hash chain continuity, behavioral sanity, mutation step parity, and gene commitment before returning the next salt and mutation order.

### Silent failure model

Invalid heartbeats are returned as `{"status":"ok"}` without mutation fields. This avoids giving attackers explicit feedback.

---

## Storage Backends

ChronoSeal supports multiple runtime storage backends configured via `db_type`:

* `sqlite-in-memory` — default ephemeral session storage
* `sqlite-disk` — persisted SQLite storage on disk
* `valkey` — alternative backend compatibility mode for future high-performance storage

---

## Deployment

ChronoSeal is intended to run as a systemd-managed Unix daemon with strict sandboxing and observable metrics.

See [docs/DEPLOYMENT.md](docs/DEPLOYMENT.md) for build, installation, and production deployment guidance.

---

## Security Model

ChronoSeal is a cost-raising attestations layer, not a perfect bot blocker.

It protects against:

* replay attacks
* session cloning
* invalid signature injection
* broken hash chain continuity
* mutation tampering
* simple synthetic mouse and browser automation

It does not attempt to protect against:

* real users acting as bots
* server-side application vulnerabilities
* fully resourced adversaries with real browsers and hardware input devices

See [docs/THREAT_MODEL.md](docs/THREAT_MODEL.md) for the full threat model.

---

## Further Reading

* [Architecture](docs/ARCHITECTURE.md)
* [API Reference](docs/API.md)
* [Deployment](docs/DEPLOYMENT.md)
* [Threat Model](docs/THREAT_MODEL.md)
* [Design Philosophy](docs/DESIGN-PHILOSOPHY.md)
* [Privacy Policy](docs/PRIVACY%20POLICY.md)
* [WASM Build](docs/WASM_BUILD.md)
* [Refactoring v0.6.0](docs/REFRACTORING-v0.6.0.md)
