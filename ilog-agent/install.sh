#!/bin/bash
set -e

INSTALL_DIR="/usr/local/bin"
CONFIG_DIR="/etc/ilog"
SERVICE_FILE="/etc/systemd/system/ilog-agent.service"
BINARY_NAME="ilog-agent"

echo "==================================="
echo "iLog Agent Installer"
echo "==================================="
echo ""

if [ "$EUID" -ne 0 ]; then 
    echo "Error: This script must be run as root (use sudo)"
    exit 1
fi

if [ ! -f "$BINARY_NAME" ]; then
    echo "Error: $BINARY_NAME binary not found in current directory"
    echo "Please build the agent first with: cargo build --release"
    echo "Then copy target/release/$BINARY_NAME to this directory"
    exit 1
fi

echo "→ Installing binary to $INSTALL_DIR..."
cp "$BINARY_NAME" "$INSTALL_DIR/$BINARY_NAME"
chmod +x "$INSTALL_DIR/$BINARY_NAME"
echo "✓ Binary installed"

echo ""
echo "→ Creating config directory at $CONFIG_DIR..."
mkdir -p "$CONFIG_DIR"

if [ ! -f "$CONFIG_DIR/config.toml" ]; then
    if [ -f "ilog.toml.example" ]; then
        echo "→ Copying example config..."
        cp ilog.toml.example "$CONFIG_DIR/config.toml"
        echo "✓ Config template created at $CONFIG_DIR/config.toml"
        echo ""
        echo "⚠️  IMPORTANT: Edit $CONFIG_DIR/config.toml with your server and token!"
    else
        echo "→ Creating minimal config..."
        cat > "$CONFIG_DIR/config.toml" << 'EOF'
[agent]
server = "your-server.com:8080"
token = "your_project_token_here"
protocol = "tcp"

[sources.file]
enabled = true
paths = [
    "/var/log/nginx/*.log",
    "/var/log/syslog"
]
EOF
        echo "✓ Config created at $CONFIG_DIR/config.toml"
        echo ""
        echo "⚠️  IMPORTANT: Edit $CONFIG_DIR/config.toml with your server and token!"
    fi
else
    echo "✓ Config already exists at $CONFIG_DIR/config.toml (not overwriting)"
fi

echo ""
echo "→ Creating systemd service..."
cat > "$SERVICE_FILE" << 'EOF'
[Unit]
Description=iLog Agent - Real-time Log Collector
Documentation=https://github.com/mati-cloud/ilog
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
User=root
ExecStart=/usr/local/bin/ilog-agent --config /etc/ilog/config.toml
Restart=always
RestartSec=5
StandardOutput=journal
StandardError=journal

# Security hardening
NoNewPrivileges=true
PrivateTmp=true
ProtectSystem=strict
ProtectHome=true
ReadWritePaths=/var/log

# Resource limits
LimitNOFILE=65536
MemoryMax=256M

[Install]
WantedBy=multi-user.target
EOF

echo "✓ Service file created at $SERVICE_FILE"

echo ""
echo "→ Reloading systemd daemon..."
systemctl daemon-reload
echo "✓ Systemd reloaded"

echo ""
echo "==================================="
echo "Installation Complete!"
echo "==================================="
echo ""
echo "Next steps:"
echo ""
echo "1. Edit configuration:"
echo "   sudo nano $CONFIG_DIR/config.toml"
echo ""
echo "2. Start the service:"
echo "   sudo systemctl start ilog-agent"
echo ""
echo "3. Enable auto-start on boot:"
echo "   sudo systemctl enable ilog-agent"
echo ""
echo "4. Check status:"
echo "   sudo systemctl status ilog-agent"
echo ""
echo "5. View logs:"
echo "   sudo journalctl -u ilog-agent -f"
echo ""
echo "==================================="
