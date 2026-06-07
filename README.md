# Mini Exchange Portfolio System

A simplified trading platform backend built in Rust, consisting of three microservices communicating over HTTP and Redis Pub/Sub.

## Architecture

```
Client
  ├── GET /symbols, /prices, /prices/{symbol}  →  Market Service  (:8080)
  ├── GET /portfolio/{userId}                  →  Portfolio Service (:8081)
  ├── POST /orders                             →  Portfolio Service (:8081)
  └── GET /orders/{orderId}                    →  Portfolio Service (:8081)

Portfolio Service
  ├── Calls Market Service for live prices
  ├── Reads/writes PostgreSQL (users, holdings, orders)
  └── Publishes audit events → Redis Pub/Sub → Audit Service (:8082)
```

**Stack:**
- `axum` — HTTP framework
- `sqlx` — PostgreSQL async with compile-time query checking
- `redis` — price cache (Market) + event channel (Portfolio→Audit)
- `reqwest` — Portfolio→Market HTTP client
- `tokio` — async runtime

---

## Prerequisites

- [Docker](https://www.docker.com/) + Docker Compose v2
- (Optional local dev) Rust 1.79+, PostgreSQL 16, Redis 7

---

## Quick start (Docker)

```bash
git clone <repo>
cd mini-exchange

docker compose up --build
```

Services will be available at:
| Service   | URL                      |
|-----------|--------------------------|
| Market    | http://localhost:8080    |
| Portfolio | http://localhost:8081    |
| Audit     | http://localhost:8082    |

---

## Local development (without Docker)

```bash
# 1. Start infra
docker compose up postgres redis -d

# 2. Copy env
cp .env.example .env

# 3. Run services in separate terminals
cargo run -p market
cargo run -p portfolio
cargo run -p audit
```

---

## API reference

### Market Service

```bash
# List tradeable symbols
curl http://localhost:8080/symbols

# All prices
curl http://localhost:8080/prices

# Single symbol
curl http://localhost:8080/prices/BTC
```

### Portfolio Service

```bash
# Get portfolio (Alice — well funded)
curl http://localhost:8081/portfolio/a0000000-0000-0000-0000-000000000001

# BUY 0.1 BTC
curl -X POST http://localhost:8081/orders \
  -H "Content-Type: application/json" \
  -d '{
    "user_id": "a0000000-0000-0000-0000-000000000001",
    "symbol": "BTC",
    "side": "BUY",
    "quantity": 0.1
  }'

# SELL 0.05 BTC
curl -X POST http://localhost:8081/orders \
  -H "Content-Type: application/json" \
  -d '{
    "user_id": "a0000000-0000-0000-0000-000000000001",
    "symbol": "BTC",
    "side": "SELL",
    "quantity": 0.05
  }'

# Get order status
curl http://localhost:8081/orders/<order_id>
```

### Audit Service

```bash
# List recent audit events
curl http://localhost:8082/audit
```

---

## Test scenarios

The scenarios below can be run manually via `curl`, or all at once with the bundled
integration test suite:

```bash
# Make sure the stack is up first
docker compose up --build

# In another terminal
chmod +x test_integration.sh
./test_integration.sh
```

`test_integration.sh` polls each service until reachable, then runs scenarios 1-7
below (plus a bonus `GET /orders/{orderId}` check) against the live stack, printing
a colored pass/fail summary with a non-zero exit code if anything fails.

### 1. Successful BUY

```bash
# Alice has $100,000; buy 0.1 BTC (~$6,500)
curl -X POST http://localhost:8081/orders \
  -H "Content-Type: application/json" \
  -d '{"user_id":"a0000000-0000-0000-0000-000000000001","symbol":"BTC","side":"BUY","quantity":0.1}'
# Expected: status "EXECUTED", portfolio cash reduced, BTC holding added
```

### 2. Successful SELL

```bash
# After buying, sell half
curl -X POST http://localhost:8081/orders \
  -H "Content-Type: application/json" \
  -d '{"user_id":"a0000000-0000-0000-0000-000000000001","symbol":"BTC","side":"SELL","quantity":0.05}'
# Expected: status "EXECUTED", cash increased, holding reduced
```

### 3. Insufficient balance (Bob has only $500)

```bash
curl -X POST http://localhost:8081/orders \
  -H "Content-Type: application/json" \
  -d '{"user_id":"b0000000-0000-0000-0000-000000000002","symbol":"BTC","side":"BUY","quantity":1}'
# Expected: status "REJECTED", reject_reason contains "Insufficient balance"
# HTTP response: 422 Unprocessable Entity
```

### 4. Insufficient holdings

```bash
# Try to sell BTC that Bob doesn't own
curl -X POST http://localhost:8081/orders \
  -H "Content-Type: application/json" \
  -d '{"user_id":"b0000000-0000-0000-0000-000000000002","symbol":"BTC","side":"SELL","quantity":1}'
# Expected: status "REJECTED", reject_reason: "Insufficient holdings"
```

### 5. Market service failure simulation

```bash
# Stop the market service
docker compose stop market

# Attempt an order
curl -X POST http://localhost:8081/orders \
  -H "Content-Type: application/json" \
  -d '{"user_id":"a0000000-0000-0000-0000-000000000001","symbol":"ETH","side":"BUY","quantity":1}'
# Expected: HTTP 503 Service Unavailable

# Restart market
docker compose start market
```

### 6. Verify portfolio update after trades

```bash
# Check portfolio before and after trades
curl http://localhost:8081/portfolio/a0000000-0000-0000-0000-000000000001
```

### 7. Audit trail

```bash
# After any order, check audit log
curl http://localhost:8082/audit
# Expected: ORDER_CREATED, ORDER_EXECUTED (or ORDER_REJECTED) entries
```

---

## Seed data

Two test users are seeded by `migrations/002_seed.sql`:

| User | ID | Cash |
|---|---|---|
| alice@example.com | `a0000000-...0001` | $100,000 |
| bob@example.com | `b0000000-...0002` | $500 (low — for rejection tests) |
| charlie@example.com | `c0000000-...0003` | $50,000 |

---

## Project structure

```
mini-exchange/
├── Cargo.toml                  ← Workspace root
├── docker-compose.yml
├── .env.example
├── AI_USAGE.md
├── migrations/
│   ├── 001_init.sql            ← Schema
│   └── 002_seed.sql            ← Test users
├── shared/
│   └── src/
│       ├── lib.rs
│       ├── models.rs           ← DTOs: Order, Portfolio, Price, …
│       ├── events.rs           ← AuditEvent enum
│       └── errors.rs           ← AppError
└── services/
    ├── market/
    │   └── src/
    │       ├── main.rs
    │       ├── config.rs
    │       ├── error.rs
    │       ├── handlers.rs
    │       └── price_store.rs  ← In-memory prices + Redis cache
    ├── portfolio/
    │   └── src/
    │       ├── main.rs
    │       ├── config.rs
    │       ├── error.rs
    │       ├── handlers.rs
    │       ├── db.rs           ← sqlx queries + transaction helpers
    │       ├── market_client.rs
    │       └── order.rs        ← BUY/SELL logic + event emitter
    └── audit/
        └── src/
            ├── main.rs         ← Redis subscriber + persist loop
            ├── config.rs
            └── handlers.rs
```
