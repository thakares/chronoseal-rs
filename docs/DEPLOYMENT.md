# ChronoSeal Deployment Guide

ChronoSeal v0.6.0 is designed for production deployment as a Unix-native daemon with hardened systemd support, lightweight WASM client runtime, and flexible backend storage.

## Prerequisites

| Tool | Minimum version | Purpose |
|---|---|---|
| Rust | 1.87 stable | Server and WASM compilation |
| wasm-pack | 0.13 | WASM build and packaging |
| Docker | 24.x | Optional container deployment |
| docker-compose | 2.x | Optional local orchestration |
| systemd | 248+ | Service management |

Install Rust: [https://rustup.rs](https://rustup.rs)
Install wasm-pack: `cargo install wasm-pack`

---

## Build Steps

### 1. Build the WASM module

```bash
rustup target add wasm32-unknown-unknown
cargo install wasm-pack
wasm-pack build wasm --target web --release
rm -rf frontend/pkg
mv wasm/pkg frontend/pkg
```

This produces the browser runtime assets required by the frontend and the server static file handler.

### 2. Build the server binary

```bash
cargo build -p server --release
```

Binary output: `target/release/server`

### 3. Convenience script

```bash
bash scripts/build.sh
```

This script builds the WASM module, moves the generated package into `frontend/pkg`, and builds the server binary.

---

## Deploying as a Native Service

ChronoSeal is intended to run as a proper Unix daemon managed by systemd.

### Install

```bash
sudo bash scripts/install.sh
```

This installer should perform the following tasks:

* create a system user for `chronoseal`
* install the server binary into `/usr/local/bin/chronoseal`
* install static frontend assets into `/opt/chronoseal/frontend`
* install `chronoseal.service` into `/etc/systemd/system/`
* enable and start the service

### Verify the service

```bash
sudo systemctl status chronoseal
sudo journalctl -u chronoseal -f
```

### Recommended runtime options

Use structured info-level logging in production:

```bash
export RUST_LOG=info
sudo systemctl restart chronoseal
```

Avoid `RUST_LOG=debug` in production because debug logs can expose internal session identifiers.

---

## systemd Integration

The supplied `chronoseal.service` is designed for hardened Unix-native operation.

Recommended service options:

* `NoNewPrivileges=true`
* `PrivateTmp=true`
* `ProtectSystem=strict`
* `ProtectHome=true`
* `ProtectKernelTunables=true`
* `ProtectKernelModules=true`
* `ProtectControlGroups=true`
* `MemoryDenyWriteExecute=true`
* `RestrictRealtime=true`
* `RestrictSUIDSGID=true`
* `SystemCallArchitectures=native`

These options reduce the host attack surface and keep the daemon constrained to its required runtime privileges.

---

## Configuration

ChronoSeal reads configuration from a TOML file, environment variables, and CLI overrides. Use `chronoseal config` to validate the effective configuration.

Example runtime configuration options:

```toml
bind = "0.0.0.0:3000"
pid_file = "/run/chronoseal.pid"
log_level = "info"
db_type = "sqlite-in-memory"
db_path = "/var/lib/chronoseal/chronoseal.db"
```

### Supported `db_type`

* `sqlite-in-memory`
* `sqlite-disk`
* `valkey`

`sqlite-in-memory` is the default and preserves ephemeral session semantics.

`sqlite-disk` will persist session state to a file specified by `db_path`.

`valkey` selects the Valkey-compatible backend mode and may be useful for future deployment scenarios.

---

## Reverse Proxy and TLS

ChronoSeal should be served over HTTPS in production. The heartbeat protocol includes timestamps and entropy data; serving that traffic in plaintext weakens security and allows easier traffic analysis.

### nginx example

```nginx
server {
    listen 443 ssl http2;
    server_name your.domain.com;

    ssl_certificate     /etc/letsencrypt/live/your.domain.com/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/your.domain.com/privkey.pem;
    ssl_protocols       TLSv1.3;
    ssl_ciphers         ECDHE-ECDSA-AES256-GCM-SHA384:ECDHE-RSA-AES256-GCM-SHA384;

    proxy_read_timeout  35s;
    proxy_send_timeout  10s;

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
    server_name your.domain.com;
    return 301 https://$host$request_uri;
}
```

### Docker deployment

```bash
docker compose up -d --build
```

The supplied `docker-compose.yml` is intended for local evaluation and development. It mounts `frontend/` and exposes port `3000`.

Note: build the WASM package before container startup, or mount a pre-built `frontend/pkg/` volume.

---

## Production Best Practices

* Use TLS termination at the perimeter
* Run ChronoSeal behind a reverse proxy or firewall
* Keep `RUST_LOG` at `info` or `warn`
* Use `systemctl` for lifecycle management
* Monitor `chronoseal` metrics with Prometheus
* Place the frontend under the same origin as the protected pages or configure CORS carefully

---

## Health and Metrics

ChronoSeal exposes runtime endpoints for health and metrics.

* `chronoseal health` — health probe
* `chronoseal metrics` — Prometheus metrics output
* `chronoseal status` — runtime status report
* `chronoseal stats` — runtime statistics

These endpoints are accessible locally from the daemon and may be proxied or scraped by monitoring infrastructure.
