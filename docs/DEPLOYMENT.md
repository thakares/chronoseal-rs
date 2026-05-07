# Deployment

## Native

```bash
cargo build -p server --release
sudo cp target/release/server /usr/local/bin/chronoseal
```

## systemd

```bash
sudo cp chronoseal.service /etc/systemd/system/

sudo systemctl daemon-reload
sudo systemctl enable chronoseal
sudo systemctl start chronoseal
```

## Docker

```bash
docker compose up -d --build
```

## Reverse Proxy

Recommended:
- nginx
- Nginx Proxy Manager
- HAProxy

Enable:
- HTTP/2
- TLS 1.3
- aggressive timeout policies
