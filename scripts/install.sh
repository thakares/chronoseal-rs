#!/bin/bash
set -euo pipefail

echo "🚀 ChronoSeal Installer"
echo "======================"

# Create system user
if ! id -u chronoseal &>/dev/null; then
    sudo useradd --system --no-create-home --shell /usr/sbin/nologin chronoseal
    echo "✓ Created chronoseal system user"
fi

# Build
echo "→ Building ChronoSeal..."
cd "$(dirname "$0")/.."
bash scripts/build.sh

# Install binary
sudo install -Dm755 target/release/chronoseal /usr/local/bin/chronoseal
echo "✓ Installed binary to /usr/local/bin/chronoseal"

# Install frontend assets
sudo mkdir -p /opt/chronoseal
sudo cp -r frontend /opt/chronoseal/
sudo chown -R chronoseal:chronoseal /opt/chronoseal
echo "✓ Installed frontend assets"

# Install systemd service
sudo cp chronoseal.service /etc/systemd/system/chronoseal.service
sudo systemctl daemon-reload
echo "✓ Installed systemd service"

# Enable and start
sudo systemctl enable --now chronoseal
echo "✓ ChronoSeal service started"

echo ""
echo "✅ ChronoSeal installed successfully!"
echo ""
echo "Useful commands:"
echo "  chronoseal status          # Check service status"
echo "  chronoseal health          # Health probe"
echo "  sudo systemctl status chronoseal"
echo "  sudo journalctl -u chronoseal -f"
echo ""
echo "To uninstall: sudo systemctl disable --now chronoseal && sudo rm /usr/local/bin/chronoseal"
