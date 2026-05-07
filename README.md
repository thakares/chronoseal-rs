# ChronoSeal

ChronoSeal is a high-security anti-automation and anti-AI-scraping framework built using Rust, WASM, cryptographic heartbeat ledgers, and behavioral attestation.

## Features

- Rust + Axum backend
- WASM runtime verification
- Ed25519 signatures
- Blake3 hash-chain continuity
- Behavioral entropy collection
- Silent anti-bot mitigation
- Adaptive trust scoring
- Stateless HTTP verification
- GPLv3 licensed

## Architecture

```text
Browser
  ├── WASM VM
  ├── Heartbeat protocol
  ├── Cryptographic ledger
  └── Behavioral attestation

Server
  ├── Session validation
  ├── Hash-chain verification
  ├── Trust scoring
  └── Adaptive mitigation
```

## Build

### Backend

```bash
cargo run -p server --release
```

### WASM

```bash
wasm-pack build wasm --target web --release
```

## License

GPLv3
