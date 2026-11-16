ALTER TABLE pokemon
    ADD COLUMN generation integer CHECK (generation > 0),
    ADD COLUMN is_starter boolean NOT NULL DEFAULT false,
    ADD COLUMN is_legendary boolean NOT NULL DEFAULT false,
    ADD COLUMN is_mythical boolean NOT NULL DEFAULT false;
