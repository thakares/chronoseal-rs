# ChronoSeal

<p align="center">
  <img src="logo/chronoseal.svg" width="220" alt="ChronoSeal Logo">
</p>

<p align="center">
  <strong>Unix-native cryptographic attestation daemon for browser session continuity.</strong>
</p>

<p align="center">
  Privacy-first | Deterministic WASM parity | Silent rejection | Low overhead
</p>

<p align="center">
  <a href="https://github.com/thakares/chronoseal-rs/blob/main/LICENSE">
    <img src="https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg" alt="License: MIT OR Apache-2.0">
  </a>
  <a href="https://github.com/thakares/chronoseal-rs">
    <img src="https://img.shields.io/badge/rust-stable%20%E2%89%A5%201.87-orange.svg" alt="Rust stable >= 1.87">
  </a>
  <a href="https://github.com/thakares/chronoseal-rs/blob/main/docs/REFRACTORING-v0.6.0.md">
    <img src="https://img.shields.io/badge/version-v1.0.2-green.svg" alt="v1.0.2">
  </a>
  <img src="https://img.shields.io/badge/wasm-rust--compiled-blueviolet.svg" alt="WASM">
</p>

---

ChronoSeal is a Linux-native cryptographic attestation service for browser session continuity and anti-automation defense. It runs as a small daemon, serves a browser WASM runtime, and validates signed heartbeat requests through deterministic server/client state progression.

The core idea is simple: a real browser session should be able to keep advancing a private cryptographic state, a hash chain, and a deterministic synthetic gene mutation chain. Basic HTTP clients, stale replay attempts, and incomplete automation should fail without receiving a useful failure reason.

ChronoSeal is a cost-raising attestation layer. It is not a CAPTCHA replacement, identity provider, fingerprinting product, or perfect bot blocker.

## Contents

- [Features](#features)
- [How It Works](#how-it-works)
- [Repository Layout](#repository-layout)
- [Quick Start](#quick-start)
- [Build From Source](#build-from-source)
- [Run Locally](#run-locally)
- [Install as a Service](#install-as-a-service)
- [Docker](#docker)
- [CLI Reference](#cli-reference)
- [Configuration](#configuration)
- [HTTP API](#http-api)
- [Browser Integration](#browser-integration)
- [Storage Backends](#storage-backends)
- [Operations](#operations)
- [Security Model](#security-model)
- [Development](#development)
- [Troubleshooting](#troubleshooting)
- [Further Reading](#further-reading)
- [License](#license)

## Features

- Native Unix daemon with `systemd` service support.
- Axum-based HTTP service exposing `/init`, `/hb`, `/health`, `/metrics`, and `/stats`.
- Rust/WASM browser runtime for key generation, signing, VM execution, and mutation preview.
- Deterministic Synthetic Gene Mutation Engine shared by the server and WASM crates.
- Ed25519 signatures over canonical heartbeat payloads.
- Blake3 hash chain continuity across accepted heartbeats.
- Silent rejection semantics: invalid heartbeats still return `{"status":"ok"}`.
- Configurable behavioral trust checks for mouse movement, pauses, timing, and browser signals.
- Runtime storage abstraction with `sqlite-in-memory`, `sqlite-in-disk`, and `valkey` modes.
- CLI-first lifecycle, status, health, metrics, stats, config validation, key generation, and shell completions.
- Privacy-oriented design based on ephemeral session state rather than long-term identity tracking.
- Security response headers (CSP, X-Frame-Options, Referrer-Policy, Permissions-Policy, X-Content-Type-Options).
- Fingerprint validation with explicit bounds checking for aspect ratio, device pixel ratio, and hardware concurrency.
- DashMap-based concurrent rate limiter internals.
- Non-root Docker container execution.
- VM stack-depth protection.
- WASM and hashing panic-resistance safeguards.
- Progressive Web App (PWA) assets including favicon and web manifest support.

✅ ~9 MiB native daemon <br/>
✅ ~728 KiB WASM runtime <br/>
✅ Full RELRO <br/>
✅ PIE enabled <br/>
✅ Stack canaries <br/>
✅ NX enabled <br/>

## How It Works

ChronoSeal creates a short-lived browser attestation session and advances it through signed heartbeats.

1. The browser loads the generated WASM package from `frontend/pkg`.
2. The WASM runtime generates an Ed25519 keypair and returns the public key to JavaScript.
3. The browser calls `POST /init` with the public key.
4. The server creates a session and returns:
   - `session_id`
   - initial salt
   - initial hash chain head
   - VM opcode program
   - gene size
   - mutation step
   - mutation order
   - heartbeat timing bounds
5. For each heartbeat, the browser:
   - executes the VM opcode program
   - collects entropy and browser signal data
   - previews the next synthetic gene commitment in WASM
   - signs the canonical heartbeat payload
   - posts the request to `POST /hb`
6. The server validates:
   - session existence and expiration
   - rate limit
   - timestamp drift
   - Ed25519 signature
   - hash chain continuity
   - behavioral trust checks
   - mutation step
   - gene commitment parity
7. If the heartbeat is accepted, the server advances session state and returns the next salt and mutation order.
8. If the heartbeat is rejected, the server returns only `{"status":"ok"}`.

The silent failure model deliberately avoids exposing which validation failed.

## Repository Layout

```text
.
├── Cargo.toml                 # Rust workspace
├── server/                    # chronoseal daemon and CLI
├── shared/                    # shared protocol, gene model, mutation engine
├── wasm/                      # WASM runtime crate
├── frontend/                  # static browser integration files
├── docs/                      # architecture, API, deployment, threat model
├── scripts/                   # build, install, release, dev helpers
├── chronoseal.service         # systemd unit
├── Dockerfile                 # container image
└── docker-compose.yml         # local container orchestration
```

Workspace members:

- `chronoseal-server`: daemon binary crate. Binary name: `chronoseal`.
- `shared`: common cryptographic and mutation logic used by server and WASM.
- `chronoseal-wasm`: browser runtime compiled with `wasm-pack`.

## Quick Start

For a full native install:

```bash
sudo bash scripts/install.sh
```

Check the daemon:

```bash
chronoseal status --format json
chronoseal health
chronoseal metrics
sudo journalctl -u chronoseal -f
```

For local development without installing:

```bash
bash scripts/build.sh
cargo run -p chronoseal-server --bin chronoseal -- run \
  --bind 127.0.0.1:3000 \
  --frontend-dir frontend
```

Then open:

```text
http://127.0.0.1:3000
```

## Build From Source

### Requirements

| Tool | Minimum | Purpose |
|---|---:|---|
| Rust | 1.87 stable | Build server, shared crate, and tests |
| wasm-pack | 0.13 | Build browser WASM package |
| wasm32 target | current stable | WASM compilation target |
| systemd | 248+ | Optional native service management |
| Docker | 24.x | Optional container workflow |
| Docker Compose | 2.x | Optional local orchestration |

Install the WASM target and `wasm-pack`:

```bash
rustup target add wasm32-unknown-unknown
cargo install wasm-pack
```

Build everything needed for the browser and server:

```bash
bash scripts/build.sh
```

That script:

1. Builds `wasm/` with `wasm-pack build --target web --release`.
2. Replaces `frontend/pkg` with the generated WASM package.
3. Builds the `chronoseal` release binary.

Manual build:

```bash
wasm-pack build wasm --target web --release
rm -rf frontend/pkg
mv wasm/pkg frontend/pkg

cargo build -p chronoseal-server --bin chronoseal --release
```

The binary is written to:

```text
target/release/chronoseal
```

## Run Locally

Run the daemon directly from Cargo:

```bash
cargo run -p chronoseal-server --bin chronoseal -- run \
  --bind 127.0.0.1:3000 \
  --frontend-dir frontend
```

Probe it:

```bash
curl http://127.0.0.1:3000/health
curl http://127.0.0.1:3000/stats
curl http://127.0.0.1:3000/metrics
```

Use the CLI wrappers:

```bash
cargo run -p chronoseal-server --bin chronoseal -- status --bind 127.0.0.1:3000
cargo run -p chronoseal-server --bin chronoseal -- health --bind 127.0.0.1:3000
cargo run -p chronoseal-server --bin chronoseal -- stats --bind 127.0.0.1:3000 --format json
```

## Install as a Service

The installer builds the project, installs the binary, copies frontend assets, installs the `systemd` unit, and starts the service.

```bash
sudo bash scripts/install.sh
```

Installed paths:

| Path | Purpose |
|---|---|
| `/usr/local/bin/chronoseal` | daemon and CLI binary |
| `/opt/chronoseal/frontend` | static frontend assets copied by installer |
| `/etc/systemd/system/chronoseal.service` | service unit |
| `/run/chronoseal.pid` | default PID file |

Service commands:

```bash
sudo systemctl status chronoseal
sudo systemctl restart chronoseal
sudo systemctl disable --now chronoseal
sudo journalctl -u chronoseal -f
```

## Docker

Build and run with Compose:

```bash
docker compose up -d --build
```

The provided container exposes port `3000`.

```bash
curl http://127.0.0.1:3000/health
```

Important: the Dockerfile copies `frontend/` from the working tree. Build the WASM package into `frontend/pkg` before building the container if you need the browser runtime inside the image:

```bash
bash scripts/build.sh
docker compose up -d --build
```

### Container Security

ChronoSeal containers run as a dedicated non-root user by default.

This reduces the impact of potential container compromise and follows container security best practices.


## CLI Reference

Run:

```bash
chronoseal --help
```

Global options:

| Option | Environment | Description |
|---|---|---|
| `--config <path>` | `CHRONOSEAL_CONFIG` | Explicit TOML config path |
| `--format <text|json|yaml>` | - | Output format for machine-readable commands |
| `--output <text|json|yaml>` | - | Alias for `--format` |
| `--log <filter>` | `CHRONOSEAL_LOG` | Tracing filter, for example `info` or `chronoseal=debug` |

Commands:

| Command | Description |
|---|---|
| `chronoseal run` | Run the daemon |
| `chronoseal status` | Check configured daemon reachability and PID state |
| `chronoseal health` | Perform an HTTP health probe |
| `chronoseal config check` | Validate and print effective configuration |
| `chronoseal generate keypair` | Generate an Ed25519 keypair |
| `chronoseal version` | Print version/build information |
| `chronoseal db-type` | List database backend support status |
| `chronoseal metrics` | Fetch Prometheus metrics from the running daemon |
| `chronoseal stats` | Fetch service statistics from the running daemon |
| `chronoseal completion <shell>` | Generate shell completions |

Examples:

```bash
chronoseal run --bind 127.0.0.1:3000 --frontend-dir frontend
chronoseal run --db-type sqlite-in-memory
chronoseal status --format json
chronoseal health --config /etc/chronoseal/config.toml
chronoseal config check --output yaml
chronoseal generate keypair --format json
chronoseal completion bash > chronoseal.bash
```

## Configuration

Configuration precedence:

1. CLI flags
2. `CHRONOSEAL_*` environment variables
3. TOML config file
4. built-in defaults

Default config discovery:

1. path from `CHRONOSEAL_CONFIG`, if it exists
2. `/etc/chronoseal/config.toml`
3. `$XDG_CONFIG_HOME/chronoseal/config.toml`
4. `~/.config/chronoseal/config.toml`

Example:

```toml
bind = "0.0.0.0:3000"
db_type = "sqlite-in-memory"
pid_file = "/run/chronoseal.pid"
db_path = "/var/lib/chronoseal/chronoseal.sqlite"
frontend_dir = "/usr/share/chronoseal/frontend"
log_file = "/var/log/chronoseal/chronoseal.jsonl"

heartbeat_min_interval_ms = 12000
heartbeat_max_interval_ms = 25000
expiration_minutes = 30
rate_limit_count = 5
rate_limit_window_secs = 10
max_timestamp_drift_ms = 30000

min_mouse_total_dist = 10.0
max_mouse_avg_speed = 2.0
min_pause_count = 1
require_mouse_activity = true

gene_size = 512
mutation_rounds = 4
```

Common environment variables:

| Variable | Description |
|---|---|
| `CHRONOSEAL_CONFIG` | Config file path |
| `CHRONOSEAL_BIND` | Bind address, for example `127.0.0.1:3000` |
| `CHRONOSEAL_DB_TYPE` | `sqlite-in-memory`, `sqlite-in-disk`, or `valkey` |
| `CHRONOSEAL_DB_PATH` | SQLite database path |
| `CHRONOSEAL_FRONTEND_DIR` | Static frontend directory served at `/` |
| `CHRONOSEAL_PID_FILE` | PID file path |
| `CHRONOSEAL_LOG` | Tracing filter |
| `CHRONOSEAL_LOG_FILE` | Optional JSON log file |
| `CHRONOSEAL_STATE_DIR` | Base state directory used for default `db_path` |
| `CHRONOSEAL_HEARTBEAT_MIN_INTERVAL_MS` | Minimum accepted heartbeat interval |
| `CHRONOSEAL_HEARTBEAT_MAX_INTERVAL_MS` | Maximum accepted heartbeat interval |
| `CHRONOSEAL_EXPIRATION_MINUTES` | Session lifetime |
| `CHRONOSEAL_RATE_LIMIT_COUNT` | Requests allowed in the rate-limit window |
| `CHRONOSEAL_RATE_LIMIT_WINDOW_SECS` | Rate-limit window length |
| `CHRONOSEAL_MAX_TIMESTAMP_DRIFT_MS` | Accepted client timestamp drift |
| `CHRONOSEAL_MIN_MOUSE_TOTAL_DIST` | Minimum mouse movement distance |
| `CHRONOSEAL_MAX_MOUSE_AVG_SPEED` | Maximum average mouse speed |
| `CHRONOSEAL_MIN_PAUSE_COUNT` | Minimum detected pause count |
| `CHRONOSEAL_REQUIRE_MOUSE_ACTIVITY` | Enable or disable mouse activity requirement |
| `CHRONOSEAL_GENE_SIZE` | Synthetic gene buffer size |
| `CHRONOSEAL_MUTATION_ROUNDS` | Mutation rounds per program |

Validate configuration:

```bash
chronoseal config check --format yaml
```

## HTTP API

ChronoSeal exposes a small HTTP surface:

| Method | Path | Purpose |
|---|---|---|
| `POST` | `/init` | Start a browser attestation session |
| `POST` | `/hb` | Submit a signed heartbeat |
| `GET` | `/health` | Health probe |
| `GET` | `/metrics` | Prometheus metrics |
| `GET` | `/stats` | Runtime statistics |
| `GET` | `/` | Static frontend files from `frontend_dir` |

### `POST /init`

Request:

```json
{
  "public_key": "hex-encoded 32-byte Ed25519 verifying key"
}
```

Response:

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

### `POST /hb`

Request:

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
    "devicePixelRatio": 2,
    "hardwareConcurrency": 8
  },
  "mutation_step": 1,
  "gene_commitment": "64-char hex",
  "signature": "128-char hex"
}
```

Accepted response:

```json
{
  "status": "ok",
  "next_salt": "32-char hex string",
  "next_mutation_step": 2,
  "next_mutation_order_b64": "base64-encoded mutation program"
}
```

Rejected response:

```json
{
  "status": "ok"
}
```

#### Fingerprint Validation

ChronoSeal validates browser fingerprint inputs before processing.

| Field | Accepted Range |
|---|---|
| aspectRatio | finite positive value |
| devicePixelRatio | > 0 |
| hardwareConcurrency | 1..=256 |

Invalid values including NaN, Infinity, negative values, malformed numeric strings, and out-of-range CPU counts are rejected.

Detailed API semantics are documented in [docs/API.md](docs/API.md).

## Browser Integration

The frontend imports the generated WASM module from `frontend/pkg`.

```js
import init, {
  generate_keypair,
  get_public_key,
  sign_message,
  compute_next_hash,
  run_program,
  init_gene_state,
  preview_gene_commitment,
  commit_gene_preview,
  discard_gene_preview,
  current_gene_commitment
} from './pkg/chronoseal_wasm.js';
```

Call `await init()` before invoking exported functions.

WASM exports:

| Function | Purpose |
|---|---|
| `generate_keypair()` | Generate a browser-local Ed25519 keypair and return public key hex |
| `get_public_key()` | Return current public key hex |
| `sign_message(msg)` | Sign a canonical UTF-8 payload and return signature hex |
| `compute_next_hash(prev, ts, entropy, stack, salt)` | Compute next Blake3 hash-chain value |
| `run_program(b64)` | Execute a base64 VM program and return stack state |
| `init_gene_state(gene_size)` | Initialize the synthetic gene buffer |
| `preview_gene_commitment(order_b64, session_id, mutation_step, rounds)` | Preview next gene commitment |
| `commit_gene_preview()` | Commit the previewed gene mutation after accepted heartbeat |
| `discard_gene_preview()` | Discard previewed mutation after rejection or error |
| `current_gene_commitment(session_id, mutation_step)` | Return current committed gene commitment |

String-returning WASM functions return an empty string on error. Boolean-returning functions indicate success or failure directly.

## Storage Backends

Set storage mode with `db_type` or `CHRONOSEAL_DB_TYPE`.

| Backend | Description |
|---|---|
| `sqlite-in-memory` | Default ephemeral session storage. State is lost on restart. |
| `sqlite-in-disk` | SQLite database persisted at `db_path`. |
| `valkey` | Valkey-compatible backend mode. |

For Valkey mode, the server reads `CHRONOSEAL_VALKEY_ADDR` (defaulting to `127.0.0.1:6666`) and establishes a thread-safe connection pool using `r2d2` and the `redis` client crate. It leverages native Valkey sets for session ID indexing and native key expiration for automatic session cleanup. If the Valkey connection fails, the server falls back to in-memory SQLite and logs a warning.

### Valkey / Redis Server Setup

To quickly run a local Valkey/Redis instance for testing or production:

```bash
# Option A: Start a local Valkey/Redis server on port 6666
valkey-server --port 6666 --bind 127.0.0.1
# Or
redis-server --port 6666 --bind 127.0.0.1

# Option B: Spin up via Docker
docker run -d --name chronoseal-valkey -p 6666:6379 valkey/valkey:latest
```

Configure ChronoSeal to use it:
```bash
export CHRONOSEAL_DB_TYPE=valkey
export CHRONOSEAL_VALKEY_ADDR=127.0.0.1:6666
```

If your Valkey or Redis server requires credentials or secure TLS:
* **Password Only**: `redis://:your_password@127.0.0.1:6666`
* **Username & Password**: `redis://your_username:your_password@127.0.0.1:6666`
* **Secure Connection (SSL/TLS)**: `rediss://your_username:your_password@secure-host.example.com:6379`

## Operations

### Health

```bash
chronoseal health
curl http://127.0.0.1:3000/health
```

### Status

```bash
chronoseal status --format json
```

### Metrics

```bash
chronoseal metrics
curl http://127.0.0.1:3000/metrics
```

### Stats

```bash
chronoseal stats --format json
curl http://127.0.0.1:3000/stats
```

### Logs

```bash
sudo journalctl -u chronoseal -f
```

Use `RUST_LOG=info` or `CHRONOSEAL_LOG=info` for normal production operation. Avoid debug logging in production because internal identifiers may appear in logs.

### Reverse Proxy

ChronoSeal should run behind TLS in production. Terminate HTTPS at a reverse proxy such as nginx, Caddy, HAProxy, or a cloud load balancer, then proxy to the local daemon.

Example nginx location:

```nginx
location / {
    proxy_pass         http://127.0.0.1:3000;
    proxy_http_version 1.1;
    proxy_set_header   Host $host;
    proxy_set_header   X-Real-IP $remote_addr;
    proxy_set_header   X-Forwarded-For $proxy_add_x_forwarded_for;
    proxy_set_header   X-Forwarded-Proto $scheme;
}
```

## Security Model

ChronoSeal is designed to raise the cost of automation and replay attacks.

It helps defend against:

- replayed heartbeats
- stale hash chain state
- forged heartbeat signatures
- mutation commitment tampering
- session cloning using only a stolen `session_id`
- simple scripted clients that do not run the WASM runtime
- basic browser automation with weak interaction simulation
- malformed browser fingerprint payloads
- invalid numeric fingerprint values
- protocol abuse through oversized fingerprint attributes

It does not claim to stop:

- real users intentionally acting as bots
- fully resourced browser farms
- attackers with complete control of a real browser and realistic input
- server-side application vulnerabilities
- account abuse outside ChronoSeal's attestation boundary
- long-term identity or fraud decisions by itself

Privacy posture:

- Sessions are short-lived.
- The private key is generated in the browser runtime and is not sent to the server.
- ChronoSeal validates continuity and plausibility rather than creating persistent user identities.
- Invalid heartbeats are rejected silently to reduce oracle feedback.

Read the full model in [docs/THREAT_MODEL.md](docs/THREAT_MODEL.md).

## Development

Run the standard checks:

```bash
cargo fmt --check
cargo check --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

Format:

```bash
cargo fmt
```

Build WASM during frontend/runtime changes:

```bash
wasm-pack build wasm --target web
rm -rf frontend/pkg
mv wasm/pkg frontend/pkg
```

Build release artifacts:

```bash
bash scripts/build.sh
```

### Validation Tests

```bash
cargo test -p chronoseal-server fingerprint
cargo test -p chronoseal-server
```

The fingerprint validation suite verifies:

- aspect ratio bounds
- device pixel ratio bounds
- hardware concurrency bounds
- malformed numeric input handling

Generate shell completion:

```bash
chronoseal completion bash > chronoseal.bash
chronoseal completion zsh > _chronoseal
```

## Troubleshooting

### Browser fails to load WASM

Rebuild the WASM package and make sure `frontend/pkg/chronoseal_wasm.js` and the `.wasm` file exist:

```bash
bash scripts/build.sh
```

The `.wasm` file must be served as `application/wasm`. The built-in static file service handles this for normal ChronoSeal deployments.

### `chronoseal health` fails

Check that the daemon is running and that the CLI is probing the correct bind address:

```bash
sudo systemctl status chronoseal
chronoseal health --bind 127.0.0.1:3000
curl http://127.0.0.1:3000/health
```

### Heartbeats return only `{"status":"ok"}`

That is the expected response for rejected heartbeats. Common causes include:

- invalid signature
- stale `prev_hash`
- stale `mutation_step`
- mismatched `gene_commitment`
- timestamp drift beyond the configured window
- insufficient mouse movement or pause data
- rate limiting
- expired session

Use local development logs and tests to debug integration issues. Avoid debug logs in production.

### SQLite disk mode cannot persist state

Ensure the daemon user can create and write the configured database path:

```bash
sudo mkdir -p /var/lib/chronoseal
sudo chown -R chronoseal:chronoseal /var/lib/chronoseal
```

Use:

```toml
db_type = "sqlite-in-disk"
db_path = "/var/lib/chronoseal/chronoseal.sqlite"
```

### Config changes do not apply

Check precedence. CLI flags override environment variables, environment variables override config files, and config files override built-in defaults.

Print the effective config:

```bash
chronoseal config check --format yaml
```


## What's New in v1.0.2

### Security Hardening

- Added security response headers.
- Hardened browser fingerprint validation.
- Improved WASM-side error handling.
- Improved hashing safety.
- Added VM stack-depth protection.

### Runtime Improvements

- Refactored rate limiter internals using DashMap.
- Improved cleanup task efficiency.
- Improved configuration validation.
- Improved deployment hardening.

### Frontend Improvements

- Added favicon and web manifest assets.
- Added CSP defense-in-depth support.
- Reduced protocol-state logging in browser consoles.

### Quality

- 38/38 server tests passing.
- Additional fingerprint validation test coverage.

## Runtime Footprint

ChronoSeal is intentionally designed to maintain a small deployment footprint while providing browser attestation, cryptographic verification, session continuity, and WASM execution capabilities.

Typical v1.0.2 release artifact sizes:

| Component                                       | Approximate Size |
| ----------------------------------------------- | ---------------: |
| Native daemon (`chronoseal`)                    |         ~9.1 MiB |
| Browser runtime (`chronoseal_wasm.wasm`)        |         ~728 KiB |
| WASM static library (`libchronoseal_wasm.rlib`) |         ~188 KiB |

Example:

```text
chronoseal
9501232 bytes
≈ 9.06 MiB

chronoseal_wasm.wasm
745569 bytes
≈ 728 KiB
```

These compact artifact sizes help:

* reduce deployment overhead
* minimize container image growth
* improve cold-start performance
* reduce browser download size
* simplify edge and self-hosted deployments

ChronoSeal intentionally avoids heavyweight runtime dependencies and large browser frameworks, allowing the complete attestation stack to remain compact while preserving functionality.

## Further Reading

- [Architecture](docs/ARCHITECTURE.md)
- [API Reference](docs/API.md)
- [Deployment Guide](docs/DEPLOYMENT.md)
- [Threat Model](docs/THREAT_MODEL.md)
- [Design Philosophy](docs/DESIGN-PHILOSOPHY.md)
- [Privacy Policy](docs/PRIVACY%20POLICY.md)
- [WASM Build Guide](docs/WASM_BUILD.md)
- [Refactoring v0.6.0](docs/REFRACTORING-v0.6.0.md)
- [Contributing](CONTRIBUTING.md)
- [Security Policy](SECURITY.md)

## License

ChronoSeal is licensed under either of:

- MIT, see [LICENSE](LICENSE)
- Apache-2.0, see [LICENSE-APACHE](LICENSE-APACHE)

at your option.
