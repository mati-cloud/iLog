-- Drop three GIN indexes that cost write amplification on every ingest but are
-- never read.
--
-- `idx_logs_body_gin` indexes `to_tsvector('english', body)`, but the search path
-- in otel.rs uses `body ILIKE '%needle%'`. A leading-wildcard LIKE cannot use a
-- tsvector GIN index, so the planner never touches it. The two JSONB GIN indexes
-- have no query path at all — no endpoint exposes attribute filtering.
--
-- On a write-hot hypertable these three indexes were pure overhead: GIN inserts
-- are relatively expensive, and `logs` had eight indexes total. When full-text
-- search lands (replacing ILIKE with `@@ plainto_tsquery`), reintroduce a GIN
-- index on `to_tsvector` — ideally on a stored generated column so the
-- expression is computed once rather than per query.
DROP INDEX IF EXISTS idx_logs_body_gin;
DROP INDEX IF EXISTS idx_logs_resource_attrs;
DROP INDEX IF EXISTS idx_logs_log_attrs;
