#!/bin/bash
set -e
echo "Building WASM..."
cd ../wasm
wasm-pack build --target web
mv pkg ../frontend/pkg
echo "Building server..."
cd ../server
cargo build --release
echo "Done."