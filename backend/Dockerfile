FROM rust:latest as builder

WORKDIR /app

# Dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

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
