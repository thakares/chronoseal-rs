#!/bin/bash
set -euo pipefail
echo "Starting server with static frontend serving..."
cd ../server
cargo run --release