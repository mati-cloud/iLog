-- Migrate to Better Auth compatibility
-- Better Auth uses TEXT for user IDs, not INTEGER

-- Drop the old users table (we're using Better Auth's user table now)
DROP TABLE IF EXISTS users CASCADE;

-- Alter service_members to use TEXT for user_id to match Better Auth
ALTER TABLE service_members DROP CONSTRAINT IF EXISTS service_members_pkey;
ALTER TABLE service_members ALTER COLUMN user_id TYPE TEXT;
ALTER TABLE service_members ADD PRIMARY KEY (service_id, user_id);

-- Add foreign key to Better Auth's user table
ALTER TABLE service_members ADD CONSTRAINT service_members_user_id_fkey 
    FOREIGN KEY (user_id) REFERENCES "user"(id) ON DELETE CASCADE;

-- Update comments
COMMENT ON COLUMN service_members.user_id IS 'References Better Auth user.id (TEXT)';
