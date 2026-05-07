# Contributing

## Requirements

- Rust stable
- wasm-pack
- NodeJS (optional frontend tooling)

## Development

```bash
cargo fmt
cargo clippy
cargo test
```

## Guidelines

- Keep security-sensitive logic inside Rust/WASM
- Avoid placing trust logic in JavaScript
- Preserve silent-failure behavior
- Maintain deterministic protocol serialization
