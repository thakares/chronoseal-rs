#!/bin/sh
set -eu

CHRONOSEAL_VERSION="${CHRONOSEAL_VERSION:-latest}"
CHRONOSEAL_INSTALL_DIR="${CHRONOSEAL_INSTALL_DIR:-/usr/local/bin}"
CHRONOSEAL_BASE_URL="${CHRONOSEAL_BASE_URL:-https://get.chronoseal.rs/releases}"

need() {
    command -v "$1" >/dev/null 2>&1 || {
        echo "chronoseal installer: missing required command: $1" >&2
        exit 1
    }
}

need uname
need mktemp
need chmod

arch="$(uname -m)"
case "$arch" in
    x86_64|amd64) target="x86_64-unknown-linux-musl" ;;
    aarch64|arm64) target="aarch64-unknown-linux-musl" ;;
    *) echo "chronoseal installer: unsupported architecture: $arch" >&2; exit 1 ;;
esac

if command -v curl >/dev/null 2>&1; then
    fetch="curl --proto =https --tlsv1.2 -fsSL"
elif command -v wget >/dev/null 2>&1; then
    fetch="wget -qO-"
else
    echo "chronoseal installer: install curl or wget" >&2
    exit 1
fi

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

url="$CHRONOSEAL_BASE_URL/$CHRONOSEAL_VERSION/chronoseal-$target.tar.gz"
echo "downloading chronoseal $CHRONOSEAL_VERSION for $target"

# shellcheck disable=SC2086
$fetch "$url" | tar -xz -C "$tmp"
chmod 0755 "$tmp/chronoseal"

if [ "$(id -u)" -eq 0 ]; then
    install -m 0755 "$tmp/chronoseal" "$CHRONOSEAL_INSTALL_DIR/chronoseal"
else
    sudo install -m 0755 "$tmp/chronoseal" "$CHRONOSEAL_INSTALL_DIR/chronoseal"
fi

echo "installed: $CHRONOSEAL_INSTALL_DIR/chronoseal"
echo "try: chronoseal --help"
