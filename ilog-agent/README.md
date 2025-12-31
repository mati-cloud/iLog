# @mati.cloud/ilog-agent

Lightweight, modular log collector for iLog written in Rust.

## Features

- **📂 File Logs** - Tail log files in real-time
- **📋 Journald** - Collect systemd journal logs
- **🐳 Docker** - Stream container logs (optional)
- **⚡ Lightweight** - ~5-10MB RAM usage
- **🔧 Modular** - Compile only what you need
- **🔐 Secure** - ChaCha20-Poly1305 encryption + token auth
- **🚀 Real-time** - Logs sent within ~10ms (no artificial batching)
- **🔌 Persistent** - Single TCP connection with LZ4 compression

## Installation

### Quick Install (VPS/Server)

Automated installation as systemd service:

```bash
# Download and extract
curl -L https://github.com/mati-cloud/ilog/releases/latest/download/ilog-agent-x86_64-unknown-linux-gnu.tar.gz | tar xz

# Run installer (creates service, config, etc.)
sudo ./install.sh

# Edit config with your server details
sudo nano /etc/ilog/config.toml

# Start the service
sudo systemctl start ilog-agent
sudo systemctl enable ilog-agent

# Check status
sudo systemctl status ilog-agent
```

The installer will:
- ✅ Install binary to `/usr/local/bin/ilog-agent`
- ✅ Create config at `/etc/ilog/config.toml`
- ✅ Set up systemd service with auto-restart
- ✅ Configure security hardening and resource limits

### Manual Installation

Download the latest release for your platform:

```bash
# x86_64 (amd64)
curl -L https://github.com/mati-cloud/ilog/releases/latest/download/ilog-agent-x86_64-unknown-linux-gnu.tar.gz | tar xz

# ARM64 (aarch64)
curl -L https://github.com/mati-cloud/ilog/releases/latest/download/ilog-agent-aarch64-unknown-linux-gnu.tar.gz | tar xz

# Move to system path
sudo mv ilog-agent /usr/local/bin/
sudo chmod +x /usr/local/bin/ilog-agent
```

### Build From Source

```bash
# Default features (file + journald)
cargo build --release

# File logs only
cargo build --release --no-default-features --features file

# All features
cargo build --release --features all

# Binary will be at: target/release/ilog-agent
```

## Configuration

### Option 1: Config File

Create `/etc/ilog/config.toml`:

```toml
[agent]
server = "ilog.company.com:8080"
token = "proj_abc123_xyz789"
protocol = "tcp"  # "tcp" (default) or "http"

[sources.file]
enabled = true
paths = ["/var/log/nginx/*.log"]

[sources.journald]
enabled = true
units = ["nginx.service"]
```

### Protocol Options

**TCP (Default)** - Raw TCP socket with encryption and compression:
- ✅ ChaCha20-Poly1305 AEAD encryption
- ✅ LZ4 compression (2-3x size reduction)
- ✅ Persistent connection (no handshake overhead)
- ✅ Sub-millisecond latency
- ✅ Automatic reconnection with exponential backoff

**HTTP** - Traditional HTTP/1.1 (requires `http` feature):
- ✅ Firewall-friendly
- ✅ Load balancer compatible
- ❌ Higher latency (~5-15ms overhead per batch)

### Option 2: Environment Variables

```bash
export ILOG_AGENT_SERVER="ilog.company.com:8080"
export ILOG_AGENT_TOKEN="proj_abc123_xyz789"
```

## Usage

### As Systemd Service (Recommended)

```bash
# Start service
sudo systemctl start ilog-agent

# Stop service
sudo systemctl stop ilog-agent

# Restart service
sudo systemctl restart ilog-agent

# Enable auto-start on boot
sudo systemctl enable ilog-agent

# Check status
sudo systemctl status ilog-agent

# View logs
sudo journalctl -u ilog-agent -f
```

### Manual Execution

```bash
# Use config file
ilog-agent --config /etc/ilog/config.toml

# Use environment variables
ILOG_AGENT_SERVER=ilog.company.com:8080 \
ILOG_AGENT_TOKEN=proj_xxx_yyy \
ilog-agent
```

## Uninstall

```bash
# Run uninstaller
sudo ./uninstall.sh

# Or manually:
sudo systemctl stop ilog-agent
sudo systemctl disable ilog-agent
sudo rm /etc/systemd/system/ilog-agent.service
sudo rm /usr/local/bin/ilog-agent
sudo rm -rf /etc/ilog
```

## Build Sizes

| Features | Binary Size | RAM Usage |
|----------|-------------|-----------|
| file | ~3MB | ~5MB |
| file + journald | ~4MB | ~8MB |
| all | ~6MB | ~12MB |

## Log Parsing

The agent automatically detects:
- ✅ JSON logs
- ✅ Common formats (nginx, apache)
- ✅ Log levels (ERROR, WARN, INFO, DEBUG)
- ✅ Timestamps

## Examples

### PHP + Nginx Server

```toml
[agent]
server = "ilog.company.com:8080"
token = "proj_myapp_token123"

[sources.file]
enabled = true
paths = [
    "/var/log/nginx/access.log",
    "/var/log/nginx/error.log",
    "/var/www/app/storage/logs/*.log"
]

[sources.journald]
enabled = true
units = ["nginx", "php8.2-fpm"]
```

### Docker-only Setup

```bash
cargo build --release --no-default-features --features docker
```

```toml
[agent]
server = "ilog.company.com:8080"
token = "proj_containers_token456"

[sources.docker]
enabled = true
containers = ["webapp", "redis", "postgres"]
```

## License

MIT
