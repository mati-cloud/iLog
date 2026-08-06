-- Retention 90d -> 3d, compression 7d -> 1d.
--
-- The original 90-day retention was set before any real ingest volume existed.
-- A single 4-node cluster tailing all pod logs produced ~25k rows/minute, which
-- makes 90 days of retention a storage problem rather than a safety margin.
--
-- The compression policy has to move with it: compressing after 7 days never
-- fired at all under a 3-day retention, since chunks were dropped four days
-- before they became eligible. Compressing after 1 day means roughly two of the
-- three retained days are stored columnar.
--
-- add_*_policy with if_not_exists => TRUE does NOT update an existing policy, it
-- silently keeps the old one. The existing policies must be removed first.

SELECT remove_retention_policy('logs', if_exists => TRUE);
SELECT remove_compression_policy('logs', if_exists => TRUE);

SELECT add_compression_policy('logs', INTERVAL '1 day', if_not_exists => TRUE);
SELECT add_retention_policy('logs', INTERVAL '3 days', if_not_exists => TRUE);
