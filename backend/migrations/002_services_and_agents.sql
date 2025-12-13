-- Add service_id to logs BEFORE enabling compression
ALTER TABLE logs ADD COLUMN IF NOT EXISTS service_id UUID;

-- Services table (multi-tenancy)
CREATE TABLE IF NOT EXISTS services (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name TEXT NOT NULL,
    slug TEXT UNIQUE NOT NULL,
    description TEXT,
    source_type VARCHAR(50) DEFAULT 'file' NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Add foreign key constraint after services table exists
DO $$ 
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint WHERE conname = 'logs_service_id_fkey'
    ) THEN
        ALTER TABLE logs ADD CONSTRAINT logs_service_id_fkey 
            FOREIGN KEY (service_id) REFERENCES services(id) ON DELETE CASCADE;
    END IF;
END $$;

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

-- Service members (RBAC)
CREATE TABLE IF NOT EXISTS service_members (
    service_id UUID NOT NULL REFERENCES services(id) ON DELETE CASCADE,
    user_id INTEGER NOT NULL,
    role TEXT NOT NULL CHECK (role IN ('owner', 'admin', 'member', 'viewer')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (service_id, user_id)
);

-- Indexes for performance
CREATE INDEX IF NOT EXISTS idx_services_source_type ON services(source_type);
CREATE INDEX IF NOT EXISTS idx_agents_service_id ON agents(service_id);
CREATE INDEX IF NOT EXISTS idx_agents_token ON agents(token);
CREATE INDEX IF NOT EXISTS idx_agents_source_type ON agents(service_id, source_type);
CREATE INDEX IF NOT EXISTS idx_service_members_user ON service_members(user_id);
CREATE INDEX IF NOT EXISTS idx_logs_service_time ON logs(service_id, time DESC);
CREATE INDEX IF NOT EXISTS idx_logs_service_name ON logs(service_id, service_name, time DESC);

-- Add check constraint for valid agent source types
ALTER TABLE agents ADD CONSTRAINT check_agent_source_type 
    CHECK (source_type IN ('all', 'docker', 'file', 'journald', 'http', 'custom'));

-- Comments for documentation
COMMENT ON TABLE services IS 'Multi-tenant services/projects for organizing logs';
COMMENT ON TABLE agents IS 'Authentication tokens for log collection agents';
COMMENT ON COLUMN agents.source_type IS 'Type of log source this agent is authorized for: all, docker, file, journald, http, custom';
COMMENT ON COLUMN agents.metadata IS 'Source-specific metadata like allowed containers, file paths, units, etc.';
COMMENT ON COLUMN services.source_type IS 'Primary log source type for this service';
