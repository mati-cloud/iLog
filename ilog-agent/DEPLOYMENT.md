# Deployment Guide - Hetzner VPS

Complete guide for deploying iLog agent on a Hetzner VPS (or any Linux server).

## Prerequisites

- Linux server (Ubuntu 20.04+, Debian 11+, or similar)
- Root or sudo access
- iLog server address and project token

## Quick Deployment

### 1. Download Agent

```bash
# SSH into your VPS
ssh root@your-vps-ip

# Download latest release
cd /tmp
curl -L https://github.com/mati-cloud/ilog/releases/latest/download/ilog-agent-x86_64-unknown-linux-gnu.tar.gz | tar xz
```

### 2. Run Installer

```bash
# Make installer executable
chmod +x install.sh

# Run installer (creates service + config)
sudo ./install.sh
```

The installer will:
- Install binary to `/usr/local/bin/ilog-agent`
- Create config template at `/etc/ilog/config.toml`
- Set up systemd service with auto-restart
- Apply security hardening

### 3. Configure

Edit the config file with your server details:

```bash
sudo nano /etc/ilog/config.toml
```

**Minimal config:**

```toml
[agent]
server = "your-ilog-server.com:8080"
token = "proj_abc123_xyz789"
protocol = "tcp"

[sources.file]
enabled = true
paths = [
    "/var/log/nginx/*.log",
    "/var/log/syslog"
]
```

### 4. Start Service

```bash
# Start the agent
sudo systemctl start ilog-agent

# Enable auto-start on boot
sudo systemctl enable ilog-agent

# Check status
sudo systemctl status ilog-agent
```

### 5. Verify

```bash
# Watch logs in real-time
sudo journalctl -u ilog-agent -f

# You should see:
# ✓ "Connected to your-server.com:8080"
# ✓ "Using TCP protocol with ChaCha20-Poly1305 encryption and LZ4 compression"
# ✓ "Successfully sent X logs"
```

## Configuration Examples

### Nginx + PHP Application

```toml
[agent]
server = "logs.company.com:8080"
token = "proj_webapp_token123"
protocol = "tcp"

[sources.file]
enabled = true
paths = [
    "/var/log/nginx/access.log",
    "/var/log/nginx/error.log",
    "/var/www/app/storage/logs/*.log"
]

[sources.journald]
enabled = true
units = ["nginx.service", "php8.2-fpm.service"]
```

### Docker Containers

```toml
[agent]
server = "logs.company.com:8080"
token = "proj_containers_token456"
protocol = "tcp"

[sources.docker]
enabled = true
containers = ["webapp", "redis", "postgres"]
```

### System Monitoring

```toml
[agent]
server = "logs.company.com:8080"
token = "proj_system_token789"
protocol = "tcp"

[sources.file]
enabled = true
paths = [
    "/var/log/syslog",
    "/var/log/auth.log",
    "/var/log/kern.log"
]

[sources.journald]
enabled = true
units = ["ssh.service", "systemd-resolved.service"]
```

## Troubleshooting

### Service won't start

```bash
# Check service status
sudo systemctl status ilog-agent

# View detailed logs
sudo journalctl -u ilog-agent -n 50

# Common issues:
# - Invalid config syntax (check with: ilog-agent --config /etc/ilog/config.toml)
# - Wrong server address or port
# - Firewall blocking outbound connections
```

### Can't connect to server

```bash
# Test TCP connection
telnet your-server.com 8080

# Check firewall rules
sudo ufw status

# If blocked, allow outbound:
sudo ufw allow out 8080/tcp
```

### Logs not appearing

```bash
# Verify file paths exist
ls -la /var/log/nginx/*.log

# Check file permissions
sudo chmod 644 /var/log/nginx/*.log

# Restart agent
sudo systemctl restart ilog-agent
```

### High memory usage

```bash
# Check current usage
systemctl status ilog-agent

# The service has a 256MB memory limit
# If exceeded, check for log file issues (huge files, rapid writes)

# Adjust limit in service file:
sudo nano /etc/systemd/system/ilog-agent.service
# Change: MemoryMax=256M to MemoryMax=512M

sudo systemctl daemon-reload
sudo systemctl restart ilog-agent
```

## Service Management

```bash
# Start
sudo systemctl start ilog-agent

# Stop
sudo systemctl stop ilog-agent

# Restart
sudo systemctl restart ilog-agent

# Status
sudo systemctl status ilog-agent

# Enable auto-start
sudo systemctl enable ilog-agent

# Disable auto-start
sudo systemctl disable ilog-agent

# View logs (live)
sudo journalctl -u ilog-agent -f

# View logs (last 100 lines)
sudo journalctl -u ilog-agent -n 100
```

## Security

The systemd service includes security hardening:

- `NoNewPrivileges=true` - Prevents privilege escalation
- `PrivateTmp=true` - Isolated /tmp directory
- `ProtectSystem=strict` - Read-only system directories
- `ProtectHome=true` - No access to home directories
- `ReadWritePaths=/var/log` - Only log directory is writable
- `LimitNOFILE=65536` - File descriptor limit
- `MemoryMax=256M` - Memory usage limit

## Updating

```bash
# Download new version
cd /tmp
curl -L https://github.com/mati-cloud/ilog/releases/latest/download/ilog-agent-x86_64-unknown-linux-gnu.tar.gz | tar xz

# Stop service
sudo systemctl stop ilog-agent

# Replace binary
sudo cp ilog-agent /usr/local/bin/ilog-agent
sudo chmod +x /usr/local/bin/ilog-agent

# Start service
sudo systemctl start ilog-agent

# Verify
sudo systemctl status ilog-agent
```

## Uninstalling

```bash
# Run uninstaller
cd /tmp
sudo ./uninstall.sh

# Or manually:
sudo systemctl stop ilog-agent
sudo systemctl disable ilog-agent
sudo rm /etc/systemd/system/ilog-agent.service
sudo systemctl daemon-reload
sudo rm /usr/local/bin/ilog-agent
sudo rm -rf /etc/ilog
```

## Performance Tuning

### For High-Volume Logs (>10k logs/sec)

Increase channel buffer in code or use multiple agents.

### For Low-Latency Requirements

The agent already uses real-time streaming (~10ms latency). No tuning needed.

### For Bandwidth Optimization

LZ4 compression is already enabled by default (2-3x reduction).

## Monitoring

### Check Agent Health

```bash
# Service status
sudo systemctl is-active ilog-agent

# Recent errors
sudo journalctl -u ilog-agent -p err -n 20

# Connection status
sudo journalctl -u ilog-agent | grep -i "connected\|failed"
```

### Resource Usage

```bash
# Memory and CPU
systemctl status ilog-agent

# Detailed stats
sudo systemctl show ilog-agent --property=MemoryCurrent,CPUUsageNSec
```

## Multiple Environments

Deploy to multiple servers with different configs:

```bash
# Production
server = "logs.prod.company.com:8080"
token = "proj_prod_token"

# Staging
server = "logs.staging.company.com:8080"
token = "proj_staging_token"

# Development
server = "logs.dev.company.com:8080"
token = "proj_dev_token"
```

## Support

- GitHub Issues: https://github.com/mati-cloud/ilog/issues
- Protocol Docs: See PROTOCOL.md
- Configuration: See ilog.toml.example
