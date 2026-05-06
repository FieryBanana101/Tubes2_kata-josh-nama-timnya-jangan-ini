FROM rust:slim

RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    curl \
    && rm -rf /var/lib/apt/lists/*

RUN cargo install --locked trunk && \
    rustup target add wasm32-unknown-unknown

WORKDIR /app
COPY . .

WORKDIR /app/src/web

EXPOSE 8080

# Serve using trunk
CMD ["trunk", "serve", "--address", "0.0.0.0", "--port", "8080", "--release"]
