CREATE TABLE users (
    id UUID PRIMARY KEY,
    api_key VARCHAR(255) NOT NULL UNIQUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_users_api_key ON users (api_key);
