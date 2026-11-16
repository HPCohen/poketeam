CREATE TABLE owner_sessions (
    id uuid PRIMARY KEY,
    owner_id uuid NOT NULL REFERENCES owners(id) ON DELETE CASCADE,
    token_hash text NOT NULL UNIQUE,
    created_at timestamptz NOT NULL,
    last_seen_at timestamptz NOT NULL,
    revoked_at timestamptz
);
CREATE INDEX owner_sessions_owner ON owner_sessions(owner_id);

-- Preserve existing cookies, inactivity deadlines and ownership UUIDs.
INSERT INTO owner_sessions(id,owner_id,token_hash,created_at,last_seen_at)
SELECT gen_random_uuid(),id,token_hash,created_at,last_seen_at FROM owners;

ALTER TABLE owners DROP COLUMN token_hash;
