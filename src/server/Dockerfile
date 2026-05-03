# --- Build Stage ---
FROM rust:slim as builder

RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY . .

# Use BuildKit cache mounts to preserve dependencies and target between builds
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/app/target \
    cargo build --release -p Tubes2_kata-josh-nama-timnya-jangan-ini_Server && \
    cp target/release/Tubes2_kata-josh-nama-timnya-jangan-ini_Server /app/server-bin

# --- Runtime Stage ---
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    libssl3 \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /app/server-bin ./server

EXPOSE 8081
CMD ["./server"]
