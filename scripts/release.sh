#!/bin/bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

"$ROOT/scripts/build.sh"

echo "Release artifacts:"
echo " - target/release/chronoseal"
echo " - frontend/ (including pkg/)"
echo " - chronoseal.service"
echo " - scripts/install.sh"
