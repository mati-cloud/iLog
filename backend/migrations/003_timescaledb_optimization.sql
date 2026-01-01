-- TimescaleDB compression policy for optimal storage
-- Compress chunks older than 7 days with aggressive compression

ALTER TABLE logs SET (
    timescaledb.compress,
    timescaledb.compress_segmentby = 'service_name',
    timescaledb.compress_orderby = 'time DESC'
);

-- Add compression policy: compress data older than 7 days
SELECT add_compression_policy('logs', INTERVAL '7 days', if_not_exists => TRUE);

-- Retention policy: drop chunks older than 90 days (adjust as needed)
SELECT add_retention_policy('logs', INTERVAL '90 days', if_not_exists => TRUE);

-- Continuous aggregate for hourly log statistics
CREATE MATERIALIZED VIEW IF NOT EXISTS logs_hourly
WITH (timescaledb.continuous) AS
SELECT 
    time_bucket('1 hour', time) AS bucket,
    service_name,
    severity_number,
    COUNT(*) AS log_count
FROM logs
GROUP BY bucket, service_name, severity_number
WITH NO DATA;

-- Refresh policy for continuous aggregate
SELECT add_continuous_aggregate_policy('logs_hourly',
    start_offset => INTERVAL '3 hours',
    end_offset => INTERVAL '1 hour',
    schedule_interval => INTERVAL '1 hour',
    if_not_exists => TRUE
);
