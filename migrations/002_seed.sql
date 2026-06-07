INSERT INTO users (id, email, cash_balance) VALUES
    ('a0000000-0000-0000-0000-000000000001', 'alice@example.com', 100000.0),
    ('b0000000-0000-0000-0000-000000000002', 'bob@example.com',   500.0), 
    ('c0000000-0000-0000-0000-000000000003', 'charlie@example.com', 50000.0)
ON CONFLICT (id) DO NOTHING;
