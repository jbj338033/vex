CREATE TYPE deployment_status AS ENUM (
    'pending',
    'building',
    'deploying',
    'running',
    'failed',
    'stopped'
);

CREATE TABLE deployments (
    id UUID PRIMARY KEY,
    app_id UUID NOT NULL REFERENCES apps (id) ON DELETE CASCADE,
    status deployment_status NOT NULL DEFAULT 'pending',
    container_id VARCHAR(255),
    image_tag VARCHAR(255),
    port INTEGER,
    error_message TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_deployments_app_id ON deployments (app_id);
