# ChronoSeal

<p align="center">
  <img src="logo/chronoseal.svg" width="220" alt="ChronoSeal Logo">
</p>

<p align="center">
  <strong>Cryptographic attestation daemon and anti-automation framework.</strong>
</p>

<p align="center">
  Privacy-preserving • Unix-native • Lightweight • WASM-powered
</p>

---

ChronoSeal is a lightweight cryptographic attestation daemon designed to raise the operational cost of browser automation, scraping, replay attacks, and synthetic interaction.

Instead of relying on:

* CAPTCHA systems
* invasive browser fingerprinting
* telemetry-heavy tracking
* persistent identifiers

ChronoSeal establishes a continuous cryptographic proof-of-runtime continuity using:

* WASM execution
* chained cryptographic heartbeats
* behavioral entropy validation
* ephemeral attestation state

while remaining completely invisible and frictionless to legitimate human users.

v0.6.0 adds a deterministic synthetic gene mutation chain (hybrid `Vec<u8>` gene + bounded environment records) to strengthen anti-replay continuity with server/WASM parity.
See [docs/REFRACTORING-v0.6.0.md](docs/REFRACTORING-v0.6.0.md) for the full refactoring details.

---

# Features

* CLI-first Unix-native architecture
* Rich operational subcommands
* Machine-readable JSON/YAML outputs
* Hardened systemd integration
* Graceful shutdown and signal handling
* One-line installation workflow
* Prometheus-compatible metrics
* WASM-based client runtime
* Ed25519 + Blake3 cryptographic chaining
* Behavioral entropy validation
* Randomized stack-machine verification
* Deterministic synthetic gene mutation chain
* Server/WASM mutation parity checks
* Silent rejection model
* SQLite-backed ephemeral sessions
* Configurable runtime DB backend selection (`db_type`)
* Connection-pooled runtime architecture
* Lightweight deployment footprint
* Docker and native deployment support

---

# Quick Start

## Install

```bash
sudo bash scripts/install.sh
```

## Check Status

```bash
chronoseal status --format json
```

## Health Probe

```bash
chronoseal health
```

## View Metrics

```bash
chronoseal metrics
```

## View Logs

```bash
sudo journalctl -u chronoseal -f
```

---

# CLI

```bash
chronoseal --help
```

## Available Commands

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

# How It Works

ChronoSeal establishes a continuous cryptographic proof-of-presence for browser sessions.

The system is inspired by heartbeat validation models used in embedded and distributed systems.

## Session Flow

```text
Browser                                    Server
  │                                           │
  │  WASM loads, generates Ed25519 keypair    │
  │  Private key never leaves WASM memory     │
  │                                           │
  ├──── POST /init  { public_key } ──────────►│
  │◄─── { session_id, salt, opcodes_b64, H0,  │
  │     mutation_step, mutation_order_b64 } ──┤
  │                                           │
  │ Every 12–25s (randomized):                │
  │  ┌─ Collect behavioral entropy            │
  │  ├─ Execute verification VM opcodes       │
  │  ├─ Preview mutation commitment           │
  │  ├─ Attach mutation_step + commitment     │
  │  ├─ Advance Blake3 hash chain             │
  │  └─ Sign payload using Ed25519            │
  │                                           │
  ├──── POST /hb  { signed_payload } ────────►│
  │◄─── { status, next_salt,                  │
  │     next_mutation_step,                   │
  │     next_mutation_order_b64 } ────────────┤
  │                                           │
  │ Invalid sessions silently rejected        │
  │ (`status=ok` without next_* fields)       │
```

The server validates:

* signature authenticity
* heartbeat continuity
* replay resistance
* mutation step parity
* mutation commitment parity
* behavioral entropy
* timestamp validity
* fingerprint sanity

---

# Security Model

## What ChronoSeal Protects Against

| Threat                   | Mechanism                             |
| ------------------------ | ------------------------------------- |
| Replay attacks           | Blake3 chained heartbeat continuity   |
| Signature forgery        | Ed25519 keypair generated inside WASM |
| Session cloning          | Ephemeral session-bound keypairs      |
| Static scraping          | Runtime participation requirements    |
| Naive browser automation | Behavioral continuity validation      |
| Timestamp replay         | Drift-window enforcement              |
| Session flooding         | Per-session rate limiting             |

---

## Silent Rejection Model

ChronoSeal intentionally avoids explicit rejection semantics.

Invalid sessions may still receive:

```json
{ "status": "ok" }
```

This prevents:

* oracle-style probing
* protocol learning
* easy automation tuning
* behavioral enumeration

---

## What ChronoSeal Does Not Claim

ChronoSeal is a cost-raising mechanism, not an impenetrable barrier.

A sufficiently motivated adversary with:

* real browsers
* genuine input devices
* enough reverse engineering effort

can eventually bypass the system.

The goal is to make automation:

* expensive
* operationally complex
* difficult to scale
* harder to replay deterministically

---

# Architecture

```text
chronoseal-rs/
├── shared/          Shared types, hash chain, gene + mutation engine
├── server/          Axum HTTP daemon
│   ├── routes/      API routes
│   ├── session.rs   Session lifecycle + mutation parity checks
│   ├── crypto.rs    Ed25519 verification
│   ├── trust.rs     Behavioral validation
│   ├── fingerprint/ Browser sanity validation
│   ├── vm.rs        Random opcode generator
│   ├── ratelimit.rs Token bucket limiter
│   ├── cleanup.rs   Session expiration lifecycle
│   └── metrics.rs   Prometheus metrics
├── wasm/            Rust → WASM runtime
│   ├── crypto.rs    Signing + hash chaining
│   ├── vm.rs        Stack-machine executor
│   └── vm_extensions.rs Gene mutation preview/commit
├── frontend/        Lightweight JS integration
├── scripts/         Build/install/dev scripts
└── docs/            Project documentation
```

---

# Stack Machine

ChronoSeal includes a lightweight randomized stack-machine execution engine.

The server generates a randomized opcode program during session initialization.

The client executes this program on every heartbeat and includes the resulting stack state in the signed payload.

This makes heartbeat payloads structurally dynamic.

## Supported Opcodes

| Opcode | Mnemonic | Effect                  |
| ------ | -------- | ----------------------- |
| `0x00` | PUSH     | Push literal            |
| `0x01` | ADD      | Wrapping addition       |
| `0x02` | SUB      | Wrapping subtraction    |
| `0x03` | MUL      | Wrapping multiplication |
| `0x04` | XOR      | Bitwise XOR             |
| `0x05` | AND      | Bitwise AND             |
| `0x06` | OR       | Bitwise OR              |
| `0x07` | ROT      | Rotate left             |
| `0x08` | NOT      | Unary inversion         |
| `0x09` | HASH     | Blake3 stack hash       |

## Mutation Opcodes (v0.6.0)

| Opcode | Mnemonic             | Effect |
| ------ | -------------------- | ------ |
| `0x23` | GENE_LOAD            | Push `gene[idx]` |
| `0x24` | GENE_STORE           | Pop and store at `gene[idx]` |
| `0x25` | MUTATE_POINT         | Apply wrapping byte delta at index |
| `0x26` | INSERT               | Insert popped byte at index |
| `0x27` | DELETE               | Delete byte at index and push removed value |
| `0x28` | TRANSCRIBE           | Push deterministic transcription hash |
| `0x29` | APPLY_MUTAGEN        | Mix environment symbol quantity into gene byte |
| `0x2A` | FINALIZE_GENE_HASH   | Push commitment-derived `u32` |
| `0x2B` | CONSUME              | Pop amount, subtract environment quantity |
| `0x2C` | PRODUCE              | Pop amount, add environment quantity |

---

# Hash Chain

ChronoSeal uses Blake3 chained continuity validation.

## Initial Hash

```text
H(0) = Blake3( session_id ║ public_key ║ salt₀ )
```

## Heartbeat Progression

```text
H(n) = Blake3(
    saltₙ₋₁ ║
    H(n-1) ║
    timestamp ║
    Blake3(entropy_json) ║
    Blake3(stack_json)
)
```

Each heartbeat depends on:

* prior continuity
* prior server-issued salt
* behavioral entropy
* VM execution result
* timestamp progression

---

# Signature Canonicalization

Heartbeat payloads are serialized into canonical key order before signing.

The server reconstructs payloads identically before:

* Ed25519 verification
* hash progression validation

This prevents:

* serialization inconsistencies
* ambiguous signing layouts
* malformed payload tricks

---

# SQLite Schema

```sql
CREATE TABLE IF NOT EXISTS sessions (
    session_id            TEXT     PRIMARY KEY,
    public_key            BLOB     NOT NULL,
    salt                  BLOB     NOT NULL,
    last_hash             BLOB     NOT NULL,
    chain_length          INTEGER  NOT NULL DEFAULT 1,
    created_at            INTEGER  NOT NULL,
    last_seen             INTEGER  NOT NULL,
    expires_at            INTEGER  NOT NULL,
    gene                  BLOB     NOT NULL DEFAULT X'',
    environment           BLOB     NOT NULL DEFAULT X'',
    pending_mutation      BLOB     NOT NULL DEFAULT X'',
    pending_mutation_step INTEGER  NOT NULL DEFAULT 0
);
```

ChronoSeal intentionally uses ephemeral session persistence.

Session continuity is designed to reset transparently.

---

# Runtime Architecture

## Server Runtime

* Rust
* Axum
* Tokio
* SQLite (`sqlite-in-memory` / `sqlite-in-disk`)
* `db_type=valkey` compatibility mode (falls back to in-memory in v0.6.0)
* `r2d2`
* `thiserror`

## Browser Runtime

* Rust → WASM
* Ed25519 signing
* Blake3 chaining
* stack-machine execution

---

# Deployment

## Recommended Installation

```bash
sudo bash scripts/install.sh
```

The installer:

* creates `chronoseal` service user
* builds release artifacts
* installs frontend assets
* deploys hardened systemd service
* enables and starts daemon

---

## Manual Installation

```bash
bash scripts/build.sh

sudo cp target/release/chronoseal /usr/local/bin/
sudo cp chronoseal.service /etc/systemd/system/

sudo systemctl daemon-reload
sudo systemctl enable --now chronoseal
```

---

## Docker

```bash
docker compose up -d --build
```

---

# Development

## Full Build

```bash
bash scripts/build.sh
```

## Development Mode

```bash
bash scripts/dev.sh
```

## Direct Execution

```bash
cargo run -p server -- run --bind 127.0.0.1:3000
```

---

# Prerequisites

* Rust stable ≥ 1.87
* `wasm-pack`

Install:

```bash
cargo install wasm-pack
```

---

# Configuration

## Precedence

```text
CLI flags > CHRONOSEAL_* environment variables > config file > defaults
```

## Default Config Locations

```text
/etc/chronoseal/config.toml
$XDG_CONFIG_HOME/chronoseal/config.toml
~/.config/chronoseal/config.toml
```

## Runtime State

```text
~/.local/state/chronoseal/
```

## Database Backend Selection (v0.6.0)

Choose backend with config, env var, or CLI flag:

* Config: `db_type = "sqlite-in-memory" | "sqlite-in-disk" | "valkey"`
* Env: `CHRONOSEAL_DB_TYPE=...`
* CLI: `chronoseal run --db-type sqlite-in-disk --db-path /var/lib/chronoseal/chronoseal.sqlite`

Inspect backend status:

```bash
chronoseal db-type --format text
```

---

# Observability

ChronoSeal exposes:

* health probes
* runtime statistics
* Prometheus metrics

## Metrics Example

```bash
chronoseal metrics
```

```text
# HELP chronoseal_sessions Active ChronoSeal sessions
# TYPE chronoseal_sessions gauge
chronoseal_sessions 1
```

---

# Lightweight Runtime

Current release artifacts:

```text
chronoseal             ~8.5 MB
chronoseal_wasm.wasm  ~719 KB
```

ChronoSeal intentionally avoids:

* heavyweight frontend frameworks
* Electron-style packaging
* telemetry-heavy dependencies
* oversized runtime models

---

# Philosophy

ChronoSeal is intentionally not:

* a surveillance framework
* invasive browser fingerprinting
* a CAPTCHA replacement
* a telemetry ecosystem

ChronoSeal is:

* a cryptographic attestation runtime
* a behavioral continuity engine
* a proof-of-runtime framework
* a lightweight Unix-native daemon

---

# License

[MIT OR Apache-2.0](LICENSE)

---

# Project

GitHub:
https://github.com/thakares/chronoseal-rs
