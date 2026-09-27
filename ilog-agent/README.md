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

### Quick Install (systemd host)

```bash
curl -fsSL https://raw.githubusercontent.com/mati-cloud/iLog/main/ilog-agent/install.sh \
  | sudo ILOG_TOKEN=agt_xxx ILOG_SERVER=ilog.example.com:8081 sh
```

The installer:
- Downloads `ilog-agent-linux-{amd64,arm64}` from the latest GitHub release and verifies it against `SHA256SUMS`
- Installs it to `/usr/local/bin/ilog-agent`
- Writes `/etc/ilog/config.toml` (server + token, mode 0600) and a starter `/etc/ilog/sources.yaml` (never overwritten)
- Runs the agent as the unprivileged `ilog` user with only `CAP_DAC_READ_SEARCH` (read any log file)
- Enables and starts `ilog-agent.service`

Pin a release with `ILOG_VERSION=v1.4.0`. Re-run to upgrade or rotate the token.
Choose what to ship by editing `/etc/ilog/sources.yaml`, then `sudo systemctl restart ilog-agent`.

### Manual Installation

```bash
ARCH=amd64  # or arm64
curl -fLO https://github.com/mati-cloud/iLog/releases/latest/download/ilog-agent-linux-$ARCH
curl -fLO https://github.com/mati-cloud/iLog/releases/latest/download/SHA256SUMS
sha256sum -c --ignore-missing SHA256SUMS
sudo install -m 0755 ilog-agent-linux-$ARCH /usr/local/bin/ilog-agent
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

Two files, both read at startup:

- `/etc/ilog/config.toml` — where to ship and as whom (see `ilog.toml.example`):

  ```toml
  [agent]
  server = "ilog.example.com:8081"   # backend TCP ingest
  token = "agt_<agent_id>_<secret>"  # from the dashboard
  ```

- `/etc/ilog/sources.yaml` — what to tail and how to parse it (`json`, `regex`, `cri`; see `config.example.yaml`).

Override paths with `--config` / `--parser`. Any config key can also come from env with prefix `ILOG`: `ILOG_AGENT_TOKEN` → `agent.token`, `ILOG_AGENT_SERVER` → `agent.server`.

Transport is TCP only: ChaCha20-Poly1305 AEAD, LZ4, one persistent connection, reconnect with exponential backoff.

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
# Defaults to /etc/ilog/config.toml and /etc/ilog/sources.yaml
ilog-agent --config ./config.toml --parser ./sources.yaml

# Token/server from env
ILOG_AGENT_SERVER=ilog.example.com:8081 ILOG_AGENT_TOKEN=agt_xxx_yyy ilog-agent
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
sudo userdel ilog
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
