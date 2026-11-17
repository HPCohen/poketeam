-- Manual operator edits are change events, not synchronization runs.
ALTER TABLE pokemon_changes ALTER COLUMN run_id DROP NOT NULL;
