# @mati.cloud/ilog-agent

Lightweight, modular log collector for iLog written in Rust.

## Features

- **📂 File Logs** - Tail log files in real-time
- **📋 Journald** - Collect systemd journal logs
- **🐳 Docker** - Stream container logs (optional)
- **⚡ Lightweight** - ~5-10MB RAM usage
- **🔧 Modular** - Compile only what you need
- **🔐 Secure** - Token-based authentication

## Installation

### Pre-built Binaries

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

### From Source

```bash
# Default features (file + journald)
cargo build --release

# File logs only
cargo build --release --no-default-features --features file

# All features
cargo build --release --features all
```

## Configuration

### Option 1: Config File

Create `/etc/ilog/config.toml`:

```toml
[agent]
server = "ilog.company.com:8080"
token = "proj_abc123_xyz789"

[sources.file]
enabled = true
paths = ["/var/log/nginx/*.log"]

[sources.journald]
enabled = true
units = ["nginx.service"]
```

### Option 2: Environment Variables

```bash
export ILOG_AGENT_SERVER="ilog.company.com:8080"
export ILOG_AGENT_TOKEN="proj_abc123_xyz789"
```

## Usage

```bash
# Use config file
ilog-agent --config /etc/ilog/config.toml

# Use environment variables
ILOG_AGENT_SERVER=ilog.company.com:8080 \
ILOG_AGENT_TOKEN=proj_xxx_yyy \
ilog-agent
```

## Systemd Service

```ini
[Unit]
Description=iLog Agent
After=network.target

[Service]
Type=simple
ExecStart=/usr/local/bin/ilog-agent --config /etc/ilog/config.toml
Restart=always
RestartSec=5

[Install]
WantedBy=multi-user.target
```

```bash
sudo systemctl enable --now ilog-agent
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
