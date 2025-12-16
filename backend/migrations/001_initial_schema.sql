-- Enable TimescaleDB extension
CREATE EXTENSION IF NOT EXISTS timescaledb;

-- Users table for authentication
CREATE TABLE IF NOT EXISTS users (
    id SERIAL PRIMARY KEY,
    username VARCHAR(255) UNIQUE NOT NULL,
    password_hash VARCHAR(255) NOT NULL,
    email VARCHAR(255) UNIQUE NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_login TIMESTAMPTZ
);

-- OpenTelemetry logs table optimized for time-series data
CREATE TABLE IF NOT EXISTS logs (
    time TIMESTAMPTZ NOT NULL,
    trace_id VARCHAR(32),
    span_id VARCHAR(16),
    trace_flags INTEGER,
    severity_text VARCHAR(20),
    severity_number INTEGER,
    service_name VARCHAR(255) NOT NULL,
    body TEXT NOT NULL,
    resource_attributes JSONB,
    log_attributes JSONB,
    scope_name VARCHAR(255),
    scope_version VARCHAR(50),
    scope_attributes JSONB
);

-- Create hypertable for time-series optimization
SELECT create_hypertable('logs', 'time', if_not_exists => TRUE);

-- Create indexes for common query patterns
CREATE INDEX IF NOT EXISTS idx_logs_service_time ON logs (service_name, time DESC);
CREATE INDEX IF NOT EXISTS idx_logs_trace_id ON logs (trace_id) WHERE trace_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_logs_severity ON logs (severity_number, time DESC);
CREATE INDEX IF NOT EXISTS idx_logs_body_gin ON logs USING GIN (to_tsvector('english', body));
CREATE INDEX IF NOT EXISTS idx_logs_resource_attrs ON logs USING GIN (resource_attributes);
CREATE INDEX IF NOT EXISTS idx_logs_log_attrs ON logs USING GIN (log_attributes);
