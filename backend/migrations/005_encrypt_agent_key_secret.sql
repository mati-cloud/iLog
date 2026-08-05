-- Agent tokens: stop storing the credential in plaintext.
--
-- The token doubles as key material: the backend must recover it to derive the
-- per-agent AEAD transport key via HKDF on every batch. A hash would break
-- ingest outright, so the secret is encrypted at rest instead, wrapped with a
-- key derived from TOKEN_ENCRYPTION_KEY.
--
-- Token format is unchanged in shape: agt_<agent_id_simple>_<key_secret>.
-- What changes is what the row holds: key_secret_encrypted (nonce||ct||tag)
-- rather than the token string.
--
-- Existing rows cannot be migrated: the plaintext column is being dropped and
-- there is no wrapping key to re-encrypt under at migration time. Dev stage, no
-- back-compat -- clear the table and reissue agents from the dashboard.
DELETE FROM agents;

-- Lookup is by agents.id, parsed from the v2 frame header. The token index
-- served the old SELECT ... WHERE token = $1 path, which no longer exists.
DROP INDEX IF EXISTS idx_agents_token;

ALTER TABLE agents DROP COLUMN token;

-- No index and no UNIQUE constraint: this column is never a lookup key, and
-- distinct nonces make equal secrets produce different ciphertext anyway.
ALTER TABLE agents ADD COLUMN key_secret_encrypted BYTEA NOT NULL;

COMMENT ON COLUMN agents.key_secret_encrypted IS
    'HKDF input for the agent transport key, ChaCha20-Poly1305 wrapped under TOKEN_ENCRYPTION_KEY. Layout: nonce(12) || ciphertext || tag.';
