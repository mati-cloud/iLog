FROM rust:latest as chef
RUN cargo install cargo-chef
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
    && rm -rf /var/lib/apt/lists/*

# Build dependencies - this layer is cached unless dependencies change
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

# Build application - only rebuilds when source changes
COPY Cargo.toml Cargo.lock* ./
COPY src ./src
COPY migrations ./migrations

# Skip sqlx compile-time verification (use runtime checks instead)
ENV SQLX_OFFLINE=false

RUN cargo build --release

FROM debian:trixie-slim

WORKDIR /app

# Runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/ilog-backend /app/ilog-backend

EXPOSE 8080

CMD ["/app/ilog-backend"]
