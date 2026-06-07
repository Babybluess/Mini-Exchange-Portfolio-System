CREATE EXTENSION IF NOT EXISTS "uuid-ossp";

DO $$ BEGIN
    CREATE TYPE order_side AS ENUM ('BUY', 'SELL');
EXCEPTION WHEN duplicate_object THEN null;
END $$;

DO $$ BEGIN
    CREATE TYPE order_status AS ENUM ('PENDING', 'EXECUTED', 'REJECTED');
EXCEPTION WHEN duplicate_object THEN null;
END $$;

CREATE TABLE IF NOT EXISTS users (
    id            UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    email         TEXT NOT NULL UNIQUE,
    cash_balance  DOUBLE PRECISION NOT NULL DEFAULT 10000.0,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS holdings (
    user_id   UUID NOT NULL REFERENCES users(id),
    symbol    TEXT NOT NULL,
    quantity  DOUBLE PRECISION NOT NULL DEFAULT 0,
    avg_cost  DOUBLE PRECISION NOT NULL DEFAULT 0,
    PRIMARY KEY (user_id, symbol)
);

CREATE TABLE IF NOT EXISTS orders (
    id            UUID PRIMARY KEY,
    user_id       UUID NOT NULL REFERENCES users(id),
    symbol        TEXT NOT NULL,
    side          order_side NOT NULL,
    quantity      DOUBLE PRECISION NOT NULL,
    price         DOUBLE PRECISION NOT NULL,
    status        order_status NOT NULL DEFAULT 'PENDING',
    reject_reason TEXT,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS orders_user_id_idx ON orders(user_id);
CREATE INDEX IF NOT EXISTS orders_created_at_idx ON orders(created_at);

CREATE TABLE IF NOT EXISTS audit_log (
    id          UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    event_type  TEXT NOT NULL,
    payload     JSONB NOT NULL DEFAULT '{}',
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS audit_log_event_type_idx ON audit_log(event_type);
CREATE INDEX IF NOT EXISTS audit_log_created_at_idx ON audit_log(created_at);
