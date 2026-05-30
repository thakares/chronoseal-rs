# ChronoSeal Operations Handbook (OPERATIONS)

This guide describes how to deploy, monitor, scale, and maintain the ChronoSeal daemon (`chronoseald`) in production environments.

---

## 1. Systemd Deployment

In single-host deployments, ChronoSeal runs as a systemd service. 

Example systemd unit file (`/etc/systemd/system/chronoseal.service`):

```ini
[Unit]
Description=ChronoSeal Attestation Daemon
After=network.target

[Service]
Type=simple
User=chronoseal
Group=chronoseal
WorkingDirectory=/var/lib/chronoseal
ExecStart=/usr/local/bin/chronoseal run --config /etc/chronoseal.toml
Restart=always
RestartSec=5
LimitNOFILE=65536

# Hardening
ProtectSystem=full
ProtectHome=true
NoNewPrivileges=true

[Install]
WantedBy=multi-user.target
```

Enable and start the service:
```bash
systemctl daemon-reload
systemctl enable --now chronoseal
```

---

## 2. Reverse Proxy & TLS Termination

Do not expose the `chronoseald` HTTP interface directly to the public internet. Run it behind a reverse proxy (e.g. Nginx, HAProxy, Envoy) that enforces TLS termination and CORS limits.

Example Nginx config (`/etc/nginx/sites-available/chronoseal.conf`):

```nginx
server {
    listen 443 ssl http2;
    server_name attestation.example.com;

    ssl_certificate /etc/letsencrypt/live/example.com/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/example.com/privkey.pem;
    ssl_protocols TLSv1.2 TLSv1.3;
    ssl_ciphers HIGH:!aNULL:!MD5;

    location / {
        proxy_pass http://127.0.0.1:8080;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
    }
}
```

---

## 3. Storage Backends & Scaling

### A. SQLite (`sqlite-in-disk`)
*   **Best For:** Single-node deployments.
*   **Configuration:** Specify a writeable path in `db_path` and set `db_type = "sqlite-in-disk"`.
*   **Operational Note:** Concurrency is limited by SQLite's single-writer database lock. Optimistic CAS reduces collisions, but high write volumes can cause queue congestion.

### B. Valkey / Redis (`valkey`)
*   **Best For:** Distributed or high-concurrency environments.
*   **Configuration:** Set `db_type = "valkey"` and specify the node addresses via `CHRONOSEAL_VALKEY_ADDR`.
*   **Horizontal Scaling:** Set up multiple `chronoseald` stateless daemon nodes. Direct all nodes to connect to the same shared Valkey cluster. This ensures session consistency across requests routed to different nodes.

---

## 4. Monitoring & Observability

### Prometheus Integration
Scrape metrics from the `/metrics` endpoint:
```yaml
scrape_configs:
  - job_name: 'chronoseal'
    static_configs:
      - targets: ['localhost:8080']
```

Key operational alerts to configure:
*   `chronoseal_verification_failures_total` rate spike: Indicates a coordinated scraping campaign, automated spoofing attempt, or misconfigured frontend app.
*   `chronoseal_storage_latency_seconds` increase: Indicates storage backend bottleneck or lock congestion.
