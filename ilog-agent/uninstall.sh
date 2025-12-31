#!/bin/bash
set -e

INSTALL_DIR="/usr/local/bin"
CONFIG_DIR="/etc/ilog"
SERVICE_FILE="/etc/systemd/system/ilog-agent.service"
BINARY_NAME="ilog-agent"

echo "==================================="
echo "iLog Agent Uninstaller"
echo "==================================="
echo ""

if [ "$EUID" -ne 0 ]; then 
    echo "Error: This script must be run as root (use sudo)"
    exit 1
fi

echo "→ Stopping service..."
if systemctl is-active --quiet ilog-agent; then
    systemctl stop ilog-agent
    echo "✓ Service stopped"
else
    echo "✓ Service not running"
fi

echo ""
echo "→ Disabling service..."
if systemctl is-enabled --quiet ilog-agent 2>/dev/null; then
    systemctl disable ilog-agent
    echo "✓ Service disabled"
else
    echo "✓ Service not enabled"
fi

echo ""
echo "→ Removing service file..."
if [ -f "$SERVICE_FILE" ]; then
    rm "$SERVICE_FILE"
    echo "✓ Service file removed"
else
    echo "✓ Service file not found"
fi

echo ""
echo "→ Reloading systemd daemon..."
systemctl daemon-reload
echo "✓ Systemd reloaded"

echo ""
echo "→ Removing binary..."
if [ -f "$INSTALL_DIR/$BINARY_NAME" ]; then
    rm "$INSTALL_DIR/$BINARY_NAME"
    echo "✓ Binary removed"
else
    echo "✓ Binary not found"
fi

echo ""
read -p "Remove config directory $CONFIG_DIR? (y/N): " -n 1 -r
echo
if [[ $REPLY =~ ^[Yy]$ ]]; then
    if [ -d "$CONFIG_DIR" ]; then
        rm -rf "$CONFIG_DIR"
        echo "✓ Config directory removed"
    else
        echo "✓ Config directory not found"
    fi
else
    echo "✓ Config directory preserved"
fi

echo ""
echo "==================================="
echo "Uninstallation Complete!"
echo "==================================="
