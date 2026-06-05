# ChronoSeal Deployment Guide

ChronoSeal is intended to run as a small Unix daemon behind TLS, with static browser assets served either by the daemon or by the same protected origin. This guide covers native, service, and container deployment.

## Deployment Model

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
sqlite-in-disk or valkey storage
```

For local evaluation, the daemon can bind directly to `0.0.0.0:3000` or `127.0.0.1:3000`.

## Requirements

| Tool | Minimum | Purpose |
|---|---:|---|
| Rust | 1.87 stable | Build server and shared crates |
| `wasm32-unknown-unknown` target | current stable | Compile WASM runtime |
| `wasm-pack` | 0.13 | Generate browser WASM package |
| systemd | 248+ | Native service management |
| Docker | 24.x | Optional container image |
| Docker Compose | 2.x | Optional local orchestration |

Install Rust from rustup, then install the WASM tooling:

```bash
rustup target add wasm32-unknown-unknown
cargo install wasm-pack
```

## Build

Use the repository build script:

```bash
bash scripts/build.sh
```

The script:

1. Builds `wasm/` with `wasm-pack build --target web --release`.
2. Replaces `frontend/pkg` with the generated package.
3. Builds the release daemon binary.

Manual equivalent:

```bash
wasm-pack build wasm --target web --release
rm -rf frontend/pkg
mv wasm/pkg frontend/pkg

cargo build -p chronoseal-server --bin chronoseal --release
```

Release binary:

```text
target/release/chronoseal
```
## Binary Hardening Verification

Before packaging or deploying ChronoSeal, verify that the release binary includes the expected platform hardening protections.

### Security Inspection

Inspect the release binary with `checksec`:

```bash
checksec file target/release/chronoseal
```

Expected protections:

```text
Full RELRO
Stack Canary Found
NX enabled
PIE Enabled
No RPATH
No RUNPATH
```

These mitigations help reduce the impact of memory corruption vulnerabilities and runtime exploitation.

### Stripped Production Binary

To verify symbol reduction and release artifact quality:

```bash
strip target/release/chronoseal -o chronoseal.stripped

nm -D chronoseal.stripped | wc -l
```

A stripped production binary should expose only a small dynamic symbol set.

Check for remaining debug sections:

```bash
readelf -S chronoseal.stripped | grep debug
```

Production artifacts should not contain `.debug_*` sections.

### Source Path Disclosure

Rust release builds may embed local source paths from the build environment.

To reduce path disclosure:

```bash
RUSTFLAGS="--remap-path-prefix=$HOME=~" \
cargo build --release
```

or:

```bash
RUSTFLAGS="--remap-path-prefix=$(pwd)=." \
cargo build --release
```

Recommended release profile:

```toml
[profile.release]
lto = true
codegen-units = 1
panic = "abort"
strip = "symbols"
```

### Runtime Verification

Start the daemon locally:

```bash
./chronoseal run --bind 127.0.0.1:8080
```

Expected startup output:

```text
INFO chronoseal daemon started bind=127.0.0.1:8080
```

Verify core endpoints:

```bash
curl http://127.0.0.1:8080/health
curl http://127.0.0.1:8080/stats
curl http://127.0.0.1:8080/metrics
```

Successful responses confirm that:

* configuration loading succeeded
* storage initialization completed
* HTTP listeners are active
* observability endpoints are operational

### PID File Permissions

When running as an unprivileged user, writing directly to `/run` may fail:

```text
could not write PID file
Permission denied
```

For local development:

```bash
chronoseal run --pid-file /tmp/chronoseal.pid
```

For production systemd deployments, prefer:

```ini
RuntimeDirectory=chronoseal
```

and:

```text
/run/chronoseal/chronoseal.pid
```

managed by systemd.

### Additional Validation

Inspect runtime dependencies:

```bash
ldd target/release/chronoseal
```

Verify ELF program headers:

```bash
readelf -l target/release/chronoseal
```

Look for:

```text
GNU_RELRO
GNU_STACK
```

Confirm binary size:

```bash
ls -lh target/release/chronoseal
```

These checks should be performed before publishing release artifacts, container images, or distribution packages.

## Native Install

The installer builds, installs, enables, and starts the service:

```bash
sudo bash scripts/install.sh
```

Installer actions:

- create the `chronoseal` system user if missing
- build WASM and server artifacts
- install `target/release/chronoseal` to `/usr/local/bin/chronoseal`
- copy `frontend/` to `/opt/chronoseal/frontend`
- install `chronoseal.service` to `/etc/systemd/system/chronoseal.service`
- reload systemd
- enable and start the service

Verify:

```bash
sudo systemctl status chronoseal
chronoseal status --format json
chronoseal health
sudo journalctl -u chronoseal -f
```

## Running Without Install

For local development:

```bash
bash scripts/build.sh
cargo run -p chronoseal-server --bin chronoseal -- run \
  --bind 127.0.0.1:3000 \
  --frontend-dir frontend
```

Probe the daemon:

```bash
curl http://127.0.0.1:3000/health
curl http://127.0.0.1:3000/stats
curl http://127.0.0.1:3000/metrics
```

## Configuration

ChronoSeal resolves configuration in this order:

1. CLI flags
2. `CHRONOSEAL_*` environment variables
3. TOML config file
4. built-in defaults

Default config discovery:

1. `CHRONOSEAL_CONFIG`, if it points to an existing file
2. `/etc/chronoseal/config.toml`
3. `$XDG_CONFIG_HOME/chronoseal/config.toml`
4. `~/.config/chronoseal/config.toml`

Validate effective configuration:

```bash
chronoseal config check --format yaml
```

Example:

```toml
bind = "127.0.0.1:3000"
db_type = "sqlite-in-disk"
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

## Storage Backends

| Backend | `db_type` | Use case |
|---|---|---|
| SQLite memory | `sqlite-in-memory` | ephemeral local or stateless deployment |
| SQLite disk | `sqlite-in-disk` | persisted session continuity across restarts |
| Valkey | `valkey` | external session storage |

For disk persistence:

```bash
sudo mkdir -p /var/lib/chronoseal
sudo chown -R chronoseal:chronoseal /var/lib/chronoseal
```

For Valkey / Redis:

ChronoSeal expects a running Valkey or Redis instance when `db_type` is set to `valkey`.

### 1. Installing Valkey or Redis
To install Valkey (the recommended open-source option) or Redis on Linux:

* **Valkey (Debian/Ubuntu)**:
  ```bash
  sudo apt-get install -y valkey-server
  ```
* **Redis (Debian/Ubuntu)**:
  ```bash
  sudo apt-get install -y redis-server
  ```

### 2. Local Setup and Startup
By default, ChronoSeal searches for Valkey/Redis on `127.0.0.1:6666`. 

You can start a local instance manually:
```bash
# Start Valkey on port 6666
valkey-server --port 6666 --bind 127.0.0.1
# Or start Redis on port 6666
redis-server --port 6666 --bind 127.0.0.1
```

Or run it via Docker:
```bash
# Run Valkey container mapping host port 6666 to container port 6379
docker run -d --name chronoseal-valkey -p 6666:6379 valkey/valkey:latest
```

### 3. Service Configuration
Configure the environment variables to point ChronoSeal to your instance:

```bash
export CHRONOSEAL_DB_TYPE=valkey
export CHRONOSEAL_VALKEY_ADDR=127.0.0.1:6666
```

#### Providing Credentials & SSL/TLS
If your Valkey/Redis server requires authentication or secure TLS/SSL, include them directly in the `CHRONOSEAL_VALKEY_ADDR` connection URL:

* **Password Only**:
  ```bash
  export CHRONOSEAL_VALKEY_ADDR=redis://:your_password@127.0.0.1:6666
  ```
* **Username & Password**:
  ```bash
  export CHRONOSEAL_VALKEY_ADDR=redis://your_username:your_password@127.0.0.1:6666
  ```
* **Secure Connection (SSL/TLS)**: Use the `rediss://` scheme prefix:
  ```bash
  export CHRONOSEAL_VALKEY_ADDR=rediss://your_username:your_password@secure-valkey-host.example.com:6379
  ```

If Valkey connection setup fails, the current implementation logs a warning and falls back to in-memory SQLite.

## systemd

The supplied service file is intended as the baseline unit. Keep the daemon under a dedicated user and restrict filesystem access to the paths it needs.

Useful commands:

```bash
sudo systemctl daemon-reload
sudo systemctl enable --now chronoseal
sudo systemctl restart chronoseal
sudo systemctl status chronoseal
sudo journalctl -u chronoseal -f
```

Recommended hardening properties include:

- `NoNewPrivileges=true`
- `PrivateTmp=true`
- `ProtectSystem=strict`
- `ProtectHome=true`
- `ProtectKernelTunables=true`
- `ProtectKernelModules=true`
- `ProtectControlGroups=true`
- `MemoryDenyWriteExecute=true`
- `RestrictRealtime=true`
- `RestrictSUIDSGID=true`
- `SystemCallArchitectures=native`

Any hardening must still allow access to:

- the binary
- frontend assets
- PID file directory
- optional log file directory
- SQLite database directory, if using `sqlite-in-disk`

## Reverse Proxy and TLS

ChronoSeal should be served over HTTPS in production. Terminate TLS at a reverse proxy or load balancer and proxy to the local daemon.

Minimal nginx example:

```nginx
server {
    listen 443 ssl http2;
    server_name example.com;

    ssl_certificate     /etc/letsencrypt/live/example.com/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/example.com/privkey.pem;

    proxy_read_timeout 35s;
    proxy_send_timeout 10s;

    location / {
        proxy_pass         http://127.0.0.1:3000;
        proxy_http_version 1.1;
        proxy_set_header   Host $host;
        proxy_set_header   X-Real-IP $remote_addr;
        proxy_set_header   X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header   X-Forwarded-Proto $scheme;
    }
}

server {
    listen 80;
    server_name example.com;
    return 301 https://$host$request_uri;
}
```

Keep `/init`, `/hb`, and frontend assets on the same origin when possible. If you split origins, configure CORS and cookie/application policy deliberately.

## Docker

Build and run:

```bash
bash scripts/build.sh
docker compose up -d --build
```

The Compose file exposes port `3000`.

```bash
curl http://127.0.0.1:3000/health
```

The Dockerfile copies `frontend/` from the working tree. Build `frontend/pkg` before building the image when the browser WASM runtime is required inside the container.

## Observability

CLI:

```bash
chronoseal status --format json
chronoseal health
chronoseal stats --format json
chronoseal metrics
```

HTTP:

```bash
curl http://127.0.0.1:3000/health
curl http://127.0.0.1:3000/stats
curl http://127.0.0.1:3000/metrics
```

Prometheus metrics:

- `chronoseal_sessions`
- `chronoseal_expired_sessions`
- `chronoseal_max_chain_length`

## Logging

Use info-level logs for production:

```bash
CHRONOSEAL_LOG=info chronoseal run
```

or with systemd:

```bash
sudo systemctl edit chronoseal
```

Avoid debug logging in production because internal identifiers may be written to logs.

## Production Checklist

- Build `frontend/pkg` before packaging.
- Serve ChronoSeal traffic over HTTPS.
- Bind the daemon to localhost behind a reverse proxy unless direct exposure is required.
- Use a dedicated service user.
- Keep debug logs disabled.
- Choose storage intentionally: `sqlite-in-memory`, `sqlite-in-disk`, or `valkey`.
- Protect SQLite and log directories with correct ownership.
- Monitor `/health`, `/stats`, and `/metrics`.
- Verify `chronoseal config check` after environment or config changes.

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

```
## v1.0.2 Deployment Notes

- Containers run as a dedicated non-root user.
- Reverse proxies should forward X-Forwarded-For or X-Real-IP.
- Security headers are enabled by default.
