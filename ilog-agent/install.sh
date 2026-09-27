#!/bin/sh
# iLog Agent installer
#
#   curl -fsSL https://raw.githubusercontent.com/mati-cloud/iLog/main/ilog-agent/install.sh \
#     | sudo ILOG_TOKEN=agt_xxx ILOG_SERVER=ilog.example.com:8081 sh
#
# Downloads the release binary for this arch, verifies it against SHA256SUMS,
# writes /etc/ilog/config.toml (0600, carries the token) and a starter
# /etc/ilog/sources.yaml, installs a systemd unit running as the unprivileged
# `ilog` user, and starts it.
#
# Idempotent: re-running upgrades the binary and rewrites config.toml (server +
# token), but never touches an existing sources.yaml.
#
# Env: ILOG_TOKEN, ILOG_SERVER (required), ILOG_VERSION (tag, default latest),
#      ILOG_REPO (default mati-cloud/iLog).
set -eu

TOKEN="${ILOG_TOKEN:-}"
SERVER="${ILOG_SERVER:-}"
VERSION="${ILOG_VERSION:-}"
REPO="${ILOG_REPO:-mati-cloud/iLog}"

BIN_PATH=/usr/local/bin/ilog-agent
CONFIG_DIR=/etc/ilog
UNIT_PATH=/etc/systemd/system/ilog-agent.service
SVC_USER=ilog

say() { printf '\033[1;36m==>\033[0m %s\n' "$*"; }
die() { printf '\033[1;31merror:\033[0m %s\n' "$*" >&2; exit 1; }

[ "$(id -u)" -eq 0 ] || die "must run as root (pipe to 'sudo sh')."
[ "$(uname -s)" = Linux ] || die "the agent is Linux-only."
command -v systemctl >/dev/null 2>&1 || die "systemd not found."
[ -n "$TOKEN" ] || die "ILOG_TOKEN is required. Mint one in the dashboard (Services -> Tokens)."
[ -n "$SERVER" ] || die "ILOG_SERVER is required, as host:port of the backend's TCP ingest (default port 8081)."
case "$TOKEN" in agt_*_*) ;; *) die "ILOG_TOKEN must look like agt_<id>_<secret>." ;; esac
case "$SERVER" in *:*) ;; *) die "ILOG_SERVER must include a port, e.g. $SERVER:8081." ;; esac

case "$(uname -m)" in
    x86_64|amd64)  ARCH=amd64 ;;
    aarch64|arm64) ARCH=arm64 ;;
    *) die "unsupported architecture: $(uname -m)" ;;
esac

if command -v curl >/dev/null 2>&1; then DL() { curl -fsSL -o "$2" "$1"; }
elif command -v wget >/dev/null 2>&1; then DL() { wget -qO "$2" "$1"; }
else die "need curl or wget."; fi

if [ -n "$VERSION" ]; then BASE="https://github.com/$REPO/releases/download/$VERSION"
else BASE="https://github.com/$REPO/releases/latest/download"; fi

BIN_NAME="ilog-agent-linux-$ARCH"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT INT TERM

say "Downloading $BASE/$BIN_NAME"
DL "$BASE/$BIN_NAME" "$TMP/$BIN_NAME" || die "download failed."
DL "$BASE/SHA256SUMS" "$TMP/SHA256SUMS" || die "SHA256SUMS download failed."
(cd "$TMP" && grep " $BIN_NAME\$" SHA256SUMS | sha256sum -c -) >/dev/null \
    || die "checksum mismatch for $BIN_NAME; refusing to install."
say "Checksum verified."

# Stop first: overwriting a running binary fails with ETXTBSY.
systemctl stop ilog-agent 2>/dev/null || true
install -m 0755 "$TMP/$BIN_NAME" "$BIN_PATH"

if ! id "$SVC_USER" >/dev/null 2>&1; then
    say "Creating system user '$SVC_USER'"
    useradd --system --no-create-home --shell /usr/sbin/nologin "$SVC_USER"
fi

mkdir -p "$CONFIG_DIR"
# umask so the token never exists on disk world-readable, even briefly.
(umask 077; cat > "$CONFIG_DIR/config.toml" <<EOF
[agent]
server = "$SERVER"
token = "$TOKEN"
EOF
)
chown "$SVC_USER:$SVC_USER" "$CONFIG_DIR/config.toml"

if [ ! -f "$CONFIG_DIR/sources.yaml" ]; then
    cat > "$CONFIG_DIR/sources.yaml" <<'EOF'
# Log sources tailed by ilog-agent. Restart after editing:
#   systemctl restart ilog-agent
# Formats: json, regex, cri. Examples: /usr/share/doc or the repo's
# ilog-agent/config.example.yaml.
sources:
  # Plain-text lines -> message body.
  - name: system
    path: /var/log/*.log
    format: regex
    pattern: '^(?P<message>.*)$'
EOF
    say "Wrote starter $CONFIG_DIR/sources.yaml (edit to choose what to ship)."
fi

# CAP_DAC_READ_SEARCH lets the unprivileged user read any log file without
# running as root; it grants nothing else. The docker source additionally needs
# the socket: add `SupplementaryGroups=docker` via `systemctl edit ilog-agent`
# (docker group is root-equivalent, so it is opt-in).
cat > "$UNIT_PATH" <<EOF
[Unit]
Description=iLog Agent
Documentation=https://github.com/$REPO
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
User=$SVC_USER
Group=$SVC_USER
ExecStart=$BIN_PATH --config $CONFIG_DIR/config.toml --parser $CONFIG_DIR/sources.yaml
Restart=always
RestartSec=5
Environment=RUST_LOG=info

AmbientCapabilities=CAP_DAC_READ_SEARCH
CapabilityBoundingSet=CAP_DAC_READ_SEARCH
NoNewPrivileges=true
PrivateTmp=true
ProtectSystem=strict
ProtectHome=true
MemoryMax=256M
LimitNOFILE=65536

[Install]
WantedBy=multi-user.target
EOF

systemctl daemon-reload
systemctl enable --now ilog-agent
say "Installed. Status: systemctl status ilog-agent"
say "Logs:      journalctl -u ilog-agent -f"
