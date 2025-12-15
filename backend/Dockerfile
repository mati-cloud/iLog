FROM rust:latest as chef

# Install sccache for compilation caching and mold for faster linking
RUN cargo install cargo-chef sccache && \
    apt-get update && \
    apt-get install -y mold && \
    rm -rf /var/lib/apt/lists/*

WORKDIR /app

FROM chef AS planner
COPY Cargo.toml Cargo.lock* ./
COPY src ./src
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder

# Install dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    mold \
    && rm -rf /var/lib/apt/lists/*

# Configure sccache and cargo optimizations
ENV RUSTC_WRAPPER=sccache \
    SCCACHE_DIR=/sccache \
    CARGO_INCREMENTAL=0 \
    CARGO_NET_GIT_FETCH_WITH_CLI=true

# Use mold linker for faster linking
ENV RUSTFLAGS="-C link-arg=-fuse-ld=mold"

# Build dependencies - this layer is cached unless dependencies change
COPY --from=planner /app/recipe.json recipe.json
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/sccache \
    cargo chef cook --release --recipe-path recipe.json && \
    sccache --show-stats

# Build application - only rebuilds when source changes
COPY Cargo.toml Cargo.lock* ./
COPY src ./src
COPY migrations ./migrations

# Skip sqlx compile-time verification (use runtime checks instead)
ENV SQLX_OFFLINE=false

RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/sccache \
    --mount=type=cache,target=/app/target \
    cargo build --release && \
    sccache --show-stats && \
    cp /app/target/release/ilog-backend /tmp/ilog-backend

FROM debian:trixie-slim

WORKDIR /app

# Runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /tmp/ilog-backend /app/ilog-backend

EXPOSE 8080

CMD ["/app/ilog-backend"]
