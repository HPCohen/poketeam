CREATE TABLE owners (
    id uuid PRIMARY KEY,
    token_hash text NOT NULL UNIQUE,
    created_at timestamptz NOT NULL,
    last_seen_at timestamptz NOT NULL
);
CREATE TABLE sprite_assets (
    hash text PRIMARY KEY,
    content_type text NOT NULL,
    bytes bytea NOT NULL
);
CREATE TABLE pokemon (
    id integer PRIMARY KEY,
    species_id integer NOT NULL,
    name text NOT NULL,
    types text[] NOT NULL CHECK (cardinality(types) BETWEEN 1 AND 2),
    stats jsonb NOT NULL,
    sprite_source text,
    sprite_hash text REFERENCES sprite_assets(hash),
    active boolean NOT NULL DEFAULT true
);
CREATE TABLE type_effectiveness (
    attacking text NOT NULL,
    defending text NOT NULL,
    multiplier double precision NOT NULL CHECK (multiplier IN (0, 0.5, 1, 2)),
    PRIMARY KEY (attacking, defending)
);
CREATE TABLE teams (
    id uuid PRIMARY KEY,
    owner_id uuid NOT NULL REFERENCES owners(id),
    name text NOT NULL CHECK (char_length(name) BETWEEN 1 AND 100),
    revision bigint NOT NULL DEFAULT 1,
    created_at timestamptz NOT NULL,
    updated_at timestamptz NOT NULL
);
CREATE INDEX teams_owner ON teams(owner_id);
CREATE TABLE team_members (
    team_id uuid NOT NULL REFERENCES teams(id) ON DELETE CASCADE,
    pokemon_id integer NOT NULL REFERENCES pokemon(id),
    position smallint NOT NULL CHECK (position BETWEEN 0 AND 5),
    PRIMARY KEY(team_id, position),
    UNIQUE(team_id, pokemon_id)
);
CREATE INDEX team_members_pokemon ON team_members(pokemon_id);
CREATE TABLE sync_runs (
    sequence bigserial UNIQUE NOT NULL,
    id uuid PRIMARY KEY,
    started_at timestamptz NOT NULL,
    finished_at timestamptz,
    outcome text NOT NULL CHECK (outcome IN ('running', 'success', 'failed', 'abandoned')),
    pokemon_count integer,
    change_count integer,
    failure_summary text
);
CREATE TABLE pokemon_changes (
    id bigserial PRIMARY KEY,
    pokemon_id integer NOT NULL REFERENCES pokemon(id),
    run_id uuid NOT NULL REFERENCES sync_runs(id),
    detected_at timestamptz NOT NULL,
    differences jsonb NOT NULL
);
CREATE INDEX pokemon_changes_recent ON pokemon_changes(detected_at, pokemon_id);
CREATE TABLE catalog_state (
    singleton boolean PRIMARY KEY DEFAULT true CHECK(singleton),
    revision bigint NOT NULL DEFAULT 0,
    last_success_at timestamptz
);
INSERT INTO catalog_state(singleton) VALUES (true);
