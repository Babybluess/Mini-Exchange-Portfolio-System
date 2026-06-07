# AI_USAGE.md

## Tools used
- **Claude Sonnet** (claude.ai) — architecture design, code scaffolding, boilerplate generation
- **Cursor** — inline code completion, refactoring suggestions within the IDE

---

## Tasks delegated to AI

AI was used as a support tool — the core design, implementation, and architectural decisions were made by the developer. AI assisted only in the following scoped roles:

| Task | Tool | Role of AI |
|---|---|---|
| Research sqlx compile-time vs. runtime query trade-offs | Claude | Summarised `query!` offline mode vs. `query` runtime API; developer chose runtime for Docker compatibility |
| Research Redis Pub/Sub vs. Streams for audit fanout | Claude | Listed trade-offs; developer decided on Pub/Sub given single-consumer audit requirement |
| Suggest missing crate dependencies (`futures-util`, `redis`, `sqlx` in shared) | Claude | Flagged gaps after build errors; developer verified and applied |
| Optimise `upsert_holding` SQL | Claude | Suggested `ON CONFLICT DO UPDATE` pattern; developer reviewed for correctness under concurrent writes |
| Generate edge-case test scenarios for order execution | Claude | Proposed cases (negative balance, zero quantity, unknown symbol, concurrent buys); developer implemented the tests |
| Review `db.rs` for race conditions | Claude | Identified missing `FOR UPDATE` in balance read; developer confirmed and added the lock |
| Docker build failure root-cause analysis | Claude | Diagnosed Rust edition incompatibility and missing `Cargo.lock`; developer applied fixes and bumped image versions |
| Update `AI_USAGE.md` | Claude | Drafted sections; developer revised for accuracy (this file) |

---

## Key prompts used

Prompts were targeted and context-specific — not open-ended code generation requests.

```
1. "I'm building a Rust workspace with axum + sqlx + redis. Compare
   sqlx::query! (compile-time checked) vs sqlx::query (runtime) for a
   Docker multi-stage build where no database is available at build time.
   What are the trade-offs and which should I use?"

2. "In my Portfolio Service, the order execution flow does:
   read cash_balance → validate → deduct inside a transaction.
   What isolation issues could arise under concurrent BUY orders
   from the same user, and what is the correct PostgreSQL fix?"

3. "My upsert_holding query currently does a SELECT then INSERT or UPDATE.
   Suggest an atomic single-query alternative in PostgreSQL that handles
   concurrent inserts without a race condition."

4. "For a single-consumer audit trail use case backed by Redis, compare
   Pub/Sub vs. Streams. I need at-least-once delivery and ordered events
   per session — which fits better and why?"

5. "Given this db.rs compile error: [error pasted], what crate dependency
   is missing from my Cargo.toml and why wasn't it caught earlier?"

6. "Write additional edge-case tests for this order execution function:
   [code pasted]. Cover: insufficient balance, zero quantity, unknown
   symbol ticker, and two concurrent BUY requests for the same user."
```

---

## What was accepted vs. modified

All core implementation was written by the developer. The table below describes where AI input was consulted and what happened to its output.

### Implemented by developer (AI not involved)
- Overall 3-service architecture and service boundary decisions
- `shared` crate type design (`Order`, `Holding`, `Portfolio`, `AppError`)
- Order execution flow in `order.rs` (validation logic, transaction scope, event publishing)
- Service startup, config loading, and graceful shutdown in each `main.rs`
- SQL migration schema and index design
- `docker-compose.yml` service wiring, health checks, and dependency ordering
- All `axum` route definitions and middleware wiring

### AI suggestion accepted unchanged
- `ON CONFLICT DO UPDATE` pattern for `upsert_holding` — reviewed and matched expected concurrent-write semantics
- Redis Pub/Sub decision over Streams — rationale matched single-consumer, fire-and-forget audit requirement
- Edge-case test scenarios list — used as a checklist; all test implementations written by developer

### AI suggestion accepted with modification
- **Missing `FOR UPDATE` in balance read** — AI correctly identified the TOCTOU risk; developer added the lock and also changed the function signature to accept `&mut Transaction` instead of `&PgPool`, enforcing correct usage at the type level (AI did not suggest the signature change)
- **`sqlx::query!` → `sqlx::query` migration** — AI recommended the switch for Docker build compatibility; developer applied it and also removed the now-redundant `OrderSide` import that AI's rewrite left behind
- **Rust image version bump** — AI diagnosed the edition 2024 incompatibility and suggested `rust:1.85`; developer tested and found 1.88 was the actual minimum required by the full dependency tree

### AI suggestion rejected or overridden
- **Error mapping in `shared`** — AI's first draft added `axum` as a dependency of `shared` to implement `IntoResponse`. Rejected: `shared` is a library crate and must not depend on a web framework. Each service implements its own error conversion locally.

---

## Example of incorrect AI output and how it was handled

### Problem: Missing `SELECT FOR UPDATE` in balance check

**Original AI-generated code:**
```rust
// db.rs — WRONG (no row lock)
pub async fn get_user_balance(pool: &PgPool, user_id: Uuid) -> Result<f64, AppError> {
    let row = sqlx::query!(
        "SELECT cash_balance FROM users WHERE id = $1",  // ← no FOR UPDATE
        user_id
    )
    .fetch_optional(pool)
    .await?
    ...
}
```

**Problem:** Without `FOR UPDATE`, two concurrent BUY orders for the same user could both read the same balance, both pass the validation check, and both deduct — resulting in a negative balance. This is a classic TOCTOU (Time-of-check / Time-of-use) race condition.

**How it was caught:** Code review of the order execution flow. Recognised the pattern from prior PostgreSQL experience — any read-then-write in a concurrent system needs explicit row locking.

**Fix applied:**
```rust
// db.rs — CORRECT
pub async fn lock_user_balance(
    tx: &mut Transaction<'_, Postgres>,
    user_id: Uuid,
) -> Result<f64, AppError> {
    let row = sqlx::query!(
        "SELECT cash_balance FROM users WHERE id = $1 FOR UPDATE",  // ← row lock
        user_id
    )
    .fetch_optional(tx.as_mut())
    .await?
    ...
}
```

The function signature also changed: it now takes a `Transaction` ref rather than a `Pool` ref, making it impossible to call outside a transaction at the type level.

### Problem: AI coupled shared crate to axum

**Original AI-generated code:**
The first draft of `shared/src/errors.rs` included:
```rust
use axum::{http::StatusCode, response::IntoResponse};

impl IntoResponse for AppError { ... }
```

**Problem:**  This forced `shared` to depend on `axum`, which is a web framework — not appropriate for a library crate that might be used in CLI tools, tests, or non-HTTP contexts.

**Fix:** Removed `IntoResponse` from `shared`. Each service implements its own `PortfolioError` / `MarketError` wrapper that converts from `AppError` and implements `IntoResponse` locally.

---

### Problem: `sqlx::query!` macros require a live database at compile time (Docker build failure)

**Original AI-generated code:**
```rust
// services/portfolio/src/db.rs — WRONG
pub async fn get_order(pool: &PgPool, order_id: Uuid) -> Result<Order, AppError> {
    sqlx::query_as!(
        Order,
        r#"SELECT id, user_id, symbol,
                  side       AS "side: OrderSide",
                  quantity, price,
                  status     AS "status: OrderStatus",
                  reject_reason, created_at
           FROM orders WHERE id = $1"#,
        order_id
    )
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("Order {} not found", order_id)))
}
```

**Problem:** `sqlx::query!` and `sqlx::query_as!` are compile-time macros that connect to a real Postgres database (via `DATABASE_URL`) to verify the SQL and infer column types. During a `docker compose --build` there is no running database, so the build fails with a connection error. The same issue applied to identical patterns across `audit/src/main.rs` and `audit/src/handlers.rs`.

Additionally, the AI-generated Dockerfiles specified `rust:1.79-slim-bookworm`, which was incompatible with transitive dependencies that required Rust edition 2024 (stabilised in 1.85+). The workspace `Cargo.lock` was also never committed, so Docker had no pinned dependency graph to work from.

Three missing crate dependencies were also omitted by AI:
- `sqlx` missing from `shared/Cargo.toml`
- `futures-util` missing from `services/audit/Cargo.toml`
- `redis` missing from `services/portfolio/Cargo.toml`

**How it was caught:** Running `docker compose up --build` failed with a cascade of errors: missing `Cargo.lock`, Rust edition gating errors, unresolved imports, and finally sqlx macro failures.

**Fix applied:**
```rust
// services/portfolio/src/db.rs — CORRECT (runtime query, no DB needed at build time)
pub async fn get_order(pool: &PgPool, order_id: Uuid) -> Result<Order, AppError> {
    let row = sqlx::query(
        r#"SELECT id, user_id, symbol, side, quantity, price, status, reject_reason, created_at
           FROM orders WHERE id = $1"#
    )
    .bind(order_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("Order {} not found", order_id)))?;

    Ok(Order {
        id: row.get("id"),
        user_id: row.get("user_id"),
        symbol: row.get("symbol"),
        side: row.get("side"),
        quantity: row.get("quantity"),
        price: row.get("price"),
        status: row.get("status"),
        reject_reason: row.get("reject_reason"),
        created_at: row.get("created_at"),
    })
}
```

All `sqlx::query!` / `sqlx::query_as!` calls were replaced with `sqlx::query` + `.bind()` + `.get()` — the unchecked runtime API that requires no database at compile time. All three Dockerfiles were updated from `rust:1.79` to `rust:1.88`, `Cargo.lock` was generated using the same image for version consistency, and the three missing crate dependencies were added to their respective `Cargo.toml` files.
