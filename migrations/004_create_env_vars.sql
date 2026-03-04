CREATE TABLE env_vars (
    id UUID PRIMARY KEY,
    app_id UUID NOT NULL REFERENCES apps (id) ON DELETE CASCADE,
    key VARCHAR(255) NOT NULL,
    value TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (app_id, key)
);

CREATE INDEX idx_env_vars_app_id ON env_vars (app_id);
