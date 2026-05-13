#!/bin/bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

echo "Building WASM..."
cd "$ROOT/wasm"
wasm-pack build --target web --release
rm -rf "$ROOT/frontend/pkg"
mv pkg "$ROOT/frontend/pkg"

echo "Building server..."
cd "$ROOT"
cargo build -p chronoseal-server --bin chronoseal --release

echo "Done."
