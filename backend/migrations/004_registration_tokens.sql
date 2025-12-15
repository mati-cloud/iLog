-- Registration tokens for initial admin setup
CREATE TABLE IF NOT EXISTS registration_tokens (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    token TEXT UNIQUE NOT NULL,
    used BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at TIMESTAMPTZ NOT NULL,
    used_at TIMESTAMPTZ,
    used_by_user_id INTEGER
);

-- Index for fast token lookup
CREATE INDEX IF NOT EXISTS idx_registration_tokens_token ON registration_tokens(token) WHERE NOT used;

-- Add is_admin flag to users table
ALTER TABLE users ADD COLUMN IF NOT EXISTS is_admin BOOLEAN NOT NULL DEFAULT FALSE;

COMMENT ON TABLE registration_tokens IS 'One-time registration tokens for initial admin setup';
COMMENT ON COLUMN registration_tokens.token IS 'Exactly 64 characters long, cryptographically secure random token';
