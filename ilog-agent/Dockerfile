# iLog agent image.
#
# Deliberately much simpler than backend/Dockerfile. The agent links statically
# against musl and needs no cargo-chef layer juggling, no OpenSSL, and no glibc
# runtime -- so the final stage carries the binary and essentially nothing else.
#
# `--features file` only: `docker` pulls in bollard and `journald` needs
# libsystemd, which cannot be linked into a static musl binary. Kubernetes log
# collection reads files under /var/log/pods, so `file` is the whole requirement.
FROM rust:alpine AS builder

RUN apk add --no-cache musl-dev

WORKDIR /app

# Dependency layer, cached until the manifests change.
COPY Cargo.toml Cargo.lock* ./
RUN mkdir src && echo 'fn main() {}' > src/main.rs && \
    cargo build --release --locked --features file && \
    rm -rf src

COPY src ./src

# Touch main.rs so cargo rebuilds it rather than reusing the stub above; without
# this the image ships a binary that does nothing.
RUN touch src/main.rs && \
    cargo build --release --locked --features file && \
    strip target/release/ilog-agent

# Alpine rather than scratch: the agent runs on every node, and when it
# misbehaves the difference between `kubectl exec ... sh` and rebuilding an image
# to debug is worth ~5MB. ca-certificates is present for any future TLS path;
# the current TCP transport is encrypted with ChaCha20-Poly1305 at the batch
# level and does not need it.
FROM alpine:3.20

RUN apk add --no-cache ca-certificates

COPY --from=builder /app/target/release/ilog-agent /usr/local/bin/ilog-agent

# Runs as root by design: pod logs are 0640 root:root inside a 0750 directory, so
# an unprivileged UID cannot read them. The DaemonSet drops all capabilities and
# mounts the log paths read-only, so this buys the agent nothing beyond the read.
USER root

ENTRYPOINT ["/usr/local/bin/ilog-agent"]
