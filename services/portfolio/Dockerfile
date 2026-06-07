FROM rust:1.88-slim-bookworm AS builder

WORKDIR /app

RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*

COPY Cargo.toml Cargo.lock ./
COPY shared/Cargo.toml shared/Cargo.toml
COPY services/market/Cargo.toml services/market/Cargo.toml
COPY services/portfolio/Cargo.toml services/portfolio/Cargo.toml
COPY services/audit/Cargo.toml services/audit/Cargo.toml

RUN mkdir -p shared/src services/market/src services/portfolio/src services/audit/src \
    && echo "pub fn main() {}" > shared/src/lib.rs \
    && echo "fn main() {}" > services/market/src/main.rs \
    && echo "fn main() {}" > services/portfolio/src/main.rs \
    && echo "fn main() {}" > services/audit/src/main.rs

RUN cargo build --release -p portfolio 2>/dev/null || true

COPY shared/ shared/
COPY services/portfolio/ services/portfolio/
COPY services/market/ services/market/
COPY migrations/ migrations/

RUN touch shared/src/lib.rs services/portfolio/src/main.rs

RUN cargo build --release -p portfolio

FROM debian:bookworm-slim AS runtime

RUN apt-get update && apt-get install -y ca-certificates libssl3 && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/portfolio /usr/local/bin/portfolio
COPY --from=builder /app/migrations /migrations

EXPOSE 8081

CMD ["portfolio"]
