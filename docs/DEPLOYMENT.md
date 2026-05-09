# ChronoSeal — Deployment Guide

## Prerequisites

| Tool | Minimum version | Purpose |
|---|---|---|
| Rust | 1.87 stable | Server + WASM compilation |
| wasm-pack | 0.13 | WASM build and packaging |
| Docker + Compose | 24 / 2.x | Container deployment |
| nginx / NPM / HAProxy | any | TLS termination, reverse proxy |

Install Rust: https://rustup.rs  
Install wasm-pack: `cargo install wasm-pack`

---

## Build

### 1. Build the WASM module

```bash
wasm-pack build wasm --target web --release
mv wasm/pkg frontend/pkg
```

This produces `frontend/pkg/antibot_wasm.js` and `frontend/pkg/antibot_wasm_bg.wasm`,
which are loaded by `frontend/main.js` at runtime.

### 2. Build the server

```bash
cargo build -p server --release
```

Binary output: `target/release/server`

### 3. Build both (convenience script)

```bash
bash scripts/build.sh
```

---

## Running

### Development

```bash
bash scripts/dev.sh
```

Runs the server with `cargo run --release`. The server serves the `frontend/`
directory statically at `/` via tower-http `ServeDir`.

Open `http://localhost:3000` in a browser. Open DevTools console — heartbeats
should appear every 12–25 seconds. No visible UI is rendered; the protection
is entirely silent.

### Production (native binary)

```bash
cargo build -p server --release
sudo cp target/release/server /usr/local/bin/chronoseal
```

Set environment variables before running:

```bash
export RUST_LOG=info      # or warn for quieter output
chronoseal
```

The server binds to `0.0.0.0:3000` by default. Place behind a reverse proxy
for TLS — do not expose port 3000 directly.

---

## systemd

### Service file

The provided `chronoseal.service` includes hardened systemd sandboxing:

```
NoNewPrivileges=true
PrivateTmp=true
ProtectSystem=strict
ProtectHome=true
ProtectKernelTunables=true
ProtectKernelModules=true
ProtectControlGroups=true
MemoryDenyWriteExecute=true
RestrictRealtime=true
RestrictSUIDSGID=true
LockPersonality=true
SystemCallArchitectures=native
```

### Install

```bash
# Create a dedicated system user
sudo useradd --system --no-create-home --shell /usr/sbin/nologin chronoseal

# Install binary and frontend
sudo cp target/release/server /usr/local/bin/chronoseal
sudo mkdir -p /opt/chronoseal/frontend
sudo cp -r frontend/ /opt/chronoseal/frontend/
sudo chown -R chronoseal:chronoseal /opt/chronoseal

# Install and enable service
sudo cp chronoseal.service /etc/systemd/system/
sudo systemctl daemon-reload
sudo systemctl enable --now chronoseal
```

### Verify

```bash
sudo systemctl status chronoseal
journalctl -u chronoseal -f
```

---

## Docker

### Build and run

```bash
docker compose up -d --build
```

### docker-compose.yml overview

```yaml
services:
  chronoseal:
    build: .
    restart: unless-stopped
    ports:
      - "3000:3000"
    environment:
      RUST_LOG: info
    tmpfs:
      - /tmp
```

The `tmpfs` mount ensures the in-memory SQLite database is never written to
disk, even if Docker's storage driver were to flush the container filesystem.

### Dockerfile stages

The Dockerfile uses a two-stage build:

1. `rust:1.87-bookworm` — compiles the server binary
2. `debian:bookworm-slim` — minimal runtime image with only `ca-certificates`

The WASM module and frontend must be built separately (wasm-pack requires a
browser toolchain not present in the server image) and mounted or copied into
the container at `/opt/chronoseal/frontend/`.

```bash
# Build WASM first
wasm-pack build wasm --target web --release
mv wasm/pkg frontend/pkg

# Then build and run the container
docker compose up -d --build
```

Or mount the pre-built frontend as a volume:

```yaml
volumes:
  - ./frontend:/opt/chronoseal/frontend:ro
```

---

## Reverse Proxy

ChronoSeal must be served over HTTPS. The heartbeat payload contains a
timestamp; if traffic is observable in plaintext, timing attacks become
easier. TLS 1.3 is strongly recommended.

### nginx

```nginx
server {
    listen 443 ssl http2;
    server_name your.domain.com;

    ssl_certificate     /etc/letsencrypt/live/your.domain.com/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/your.domain.com/privkey.pem;
    ssl_protocols       TLSv1.3;
    ssl_ciphers         ECDHE-ECDSA-AES256-GCM-SHA384:ECDHE-RSA-AES256-GCM-SHA384;

    # Tight timeouts — heartbeat interval is 12–25s
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

### Nginx Proxy Manager

1. Add a new Proxy Host pointing to `http://chronoseal:3000`
2. Enable SSL, Request Let's Encrypt certificate
3. Enable HTTP/2, Force SSL
4. Under Advanced, add:
   ```
   proxy_read_timeout 35s;
   proxy_send_timeout 10s;
   ```

### HAProxy

```haproxy
frontend https_front
    bind *:443 ssl crt /etc/haproxy/certs/your.domain.pem alpn h2,http/1.1
    default_backend chronoseal_back

backend chronoseal_back
    server chronoseal 127.0.0.1:3000 check
    timeout connect 5s
    timeout server  35s
```

---

## Integration into an Existing Site

ChronoSeal is designed to run as a sidecar — its `/init` and `/hb` endpoints
can be proxied from any existing web server. The frontend assets (`pkg/`) need
to be served from the same origin as the protected page (or CORS must be
configured).

### Option A — Serve everything from ChronoSeal

ChronoSeal serves `frontend/` statically. Put your protected HTML inside
`frontend/` and let ChronoSeal serve it directly.

### Option B — Proxy only the API endpoints

Keep your existing server. Proxy `/init` and `/hb` to ChronoSeal, and serve
the WASM and JS assets from your CDN or existing static file server.

```nginx
# On your existing server:
location ~ ^/(init|hb)$ {
    proxy_pass http://127.0.0.1:3000;
}
```

Add to your protected pages:

```html
<script type="module" src="/pkg/antibot_wasm.js"></script>
<script type="module" src="/main.js"></script>
```

---

## Configuration

All parameters are in `shared/src/constants.rs`. Recompile after changes.

| Constant | Default | Notes |
|---|---|---|
| `SESSION_ID_LEN` | 32 bytes | 256-bit entropy — do not reduce |
| `SALT_LEN` | 16 bytes | Per-heartbeat salt |
| `HEARTBEAT_MIN_INTERVAL_MS` | 12 000 ms | Increase to reduce server load |
| `HEARTBEAT_MAX_INTERVAL_MS` | 25 000 ms | Jitter upper bound |
| `EXPIRATION_MINUTES` | 30 min | Session TTL after last heartbeat |
| `RATE_LIMIT_COUNT` | 5 | Max heartbeats per window per session |
| `RATE_LIMIT_WINDOW_SECS` | 10 s | Rate limit window |
| `MAX_TIMESTAMP_DRIFT_MS` | 30 000 ms | Anti-replay window; account for NTP skew |
| `MIN_MOUSE_TOTAL_DIST` | 10.0 px | Lower for low-activity pages |
| `MAX_MOUSE_AVG_SPEED` | 2.0 px/ms | Raise if legitimate users are rejected |
| `MIN_PAUSE_COUNT` | 1 | Minimum natural pause events |

---

## Observability

ChronoSeal uses `tracing` with `tracing-subscriber`. Log levels:

| Level | Events |
|---|---|
| `INFO` | Server start, request method + path + status |
| `WARN` | Heartbeat validation failures (with session ID and reason) |
| `DEBUG` | Rate limit hits |

```bash
RUST_LOG=info   chronoseal   # production
RUST_LOG=debug  chronoseal   # development
RUST_LOG=warn   chronoseal   # minimal output
```

Log format is plain text to stdout. Pipe to `journald`, `fluentd`, or any
log aggregator via stdout capture.

---

## Health Check

The server has no dedicated `/health` endpoint. Use a TCP check on port 3000,
or a lightweight HTTP check on `GET /` (which serves `index.html`).

```bash
# Docker health check (add to docker-compose.yml if needed)
healthcheck:
  test: ["CMD", "curl", "-sf", "http://localhost:3000/"]
  interval: 30s
  timeout: 5s
  retries: 3
```

---

## Security Checklist

- [ ] TLS 1.3 enabled, TLS 1.0/1.1 disabled
- [ ] HTTP/2 enabled
- [ ] Port 3000 not exposed to the public internet (only via reverse proxy)
- [ ] `RUST_LOG=warn` or `info` in production (not `debug` — session IDs appear in logs)
- [ ] systemd service running as `chronoseal` user with hardened sandbox
- [ ] `MemoryDenyWriteExecute=true` in service file (prevents JIT in process)
- [ ] CORS `CorsLayer::permissive()` replaced with origin-restricted policy for production
- [ ] Frontend assets served over the same HTTPS origin as protected pages
