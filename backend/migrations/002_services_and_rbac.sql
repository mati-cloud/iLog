-- Services table (multi-tenancy)
CREATE TABLE IF NOT EXISTS services (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name TEXT NOT NULL,
    slug TEXT UNIQUE NOT NULL,
    description TEXT,
    owner_id TEXT,
    source_type VARCHAR(50) DEFAULT 'file' NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Service members (RBAC)
CREATE TABLE IF NOT EXISTS service_members (
    service_id UUID NOT NULL REFERENCES services(id) ON DELETE CASCADE,
    user_id TEXT NOT NULL,
    role TEXT NOT NULL CHECK (role IN ('owner', 'admin', 'member', 'viewer')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (service_id, user_id)
);

-- Agents table (authentication tokens for log collection)
CREATE TABLE IF NOT EXISTS agents (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    service_id UUID NOT NULL REFERENCES services(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    token TEXT UNIQUE NOT NULL,
    source_type TEXT DEFAULT 'all' NOT NULL,
    metadata JSONB DEFAULT '{}',
    expires_at TIMESTAMPTZ,
    last_used_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Registration tokens for controlled user signup
CREATE TABLE IF NOT EXISTS registration_tokens (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    token TEXT UNIQUE NOT NULL,
    created_by TEXT,
    expires_at TIMESTAMPTZ,
    max_uses INTEGER DEFAULT 1,
    used_count INTEGER DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Add foreign key constraints to Better Auth user table (only if user table exists)
DO $$
BEGIN
    IF EXISTS (SELECT FROM pg_tables WHERE schemaname = 'public' AND tablename = 'user') THEN
        ALTER TABLE services ADD CONSTRAINT services_owner_id_fkey 
            FOREIGN KEY (owner_id) REFERENCES "user"(id) ON DELETE SET NULL;
        
        ALTER TABLE service_members ADD CONSTRAINT service_members_user_id_fkey 
            FOREIGN KEY (user_id) REFERENCES "user"(id) ON DELETE CASCADE;
        
        ALTER TABLE registration_tokens ADD CONSTRAINT registration_tokens_created_by_fkey 
            FOREIGN KEY (created_by) REFERENCES "user"(id) ON DELETE SET NULL;
    END IF;
END $$;

-- Add foreign key for logs to services
ALTER TABLE logs ADD CONSTRAINT logs_service_id_fkey 
    FOREIGN KEY (service_id) REFERENCES services(id) ON DELETE CASCADE;

-- Indexes for performance
CREATE INDEX IF NOT EXISTS idx_services_owner_id ON services(owner_id);
CREATE INDEX IF NOT EXISTS idx_services_source_type ON services(source_type);
CREATE INDEX IF NOT EXISTS idx_service_members_user ON service_members(user_id);
CREATE INDEX IF NOT EXISTS idx_agents_service_id ON agents(service_id);
CREATE INDEX IF NOT EXISTS idx_agents_token ON agents(token);
CREATE INDEX IF NOT EXISTS idx_agents_source_type ON agents(service_id, source_type);
CREATE INDEX IF NOT EXISTS idx_registration_tokens_token ON registration_tokens(token);
CREATE INDEX IF NOT EXISTS idx_registration_tokens_expires ON registration_tokens(expires_at);

-- Add check constraint for valid agent source types
ALTER TABLE agents ADD CONSTRAINT check_agent_source_type 
    CHECK (source_type IN ('all', 'docker', 'file', 'journald', 'http', 'custom'));

-- Add check constraint for registration token usage
ALTER TABLE registration_tokens ADD CONSTRAINT check_token_usage 
    CHECK (used_count <= max_uses);

-- Comments for documentation
COMMENT ON TABLE services IS 'Multi-tenant services/projects for organizing logs';
COMMENT ON TABLE service_members IS 'RBAC for service access - links users to services with roles';
COMMENT ON TABLE agents IS 'Authentication tokens for log collection agents';
COMMENT ON TABLE registration_tokens IS 'Tokens for controlled user registration';
COMMENT ON COLUMN services.owner_id IS 'References Better Auth user.id (TEXT) - the user who created this service';
COMMENT ON COLUMN service_members.user_id IS 'References Better Auth user.id (TEXT)';
COMMENT ON COLUMN agents.source_type IS 'Type of log source this agent is authorized for: all, docker, file, journald, http, custom';
COMMENT ON COLUMN agents.metadata IS 'Source-specific metadata like allowed containers, file paths, units, etc.';
COMMENT ON COLUMN services.source_type IS 'Primary log source type for this service';
