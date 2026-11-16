ALTER TABLE pokemon ADD COLUMN sprite_variants jsonb NOT NULL DEFAULT '{}'::jsonb;
UPDATE pokemon SET sprite_variants=jsonb_build_object('front_default',sprite_source)
WHERE sprite_source IS NOT NULL;

ALTER TABLE team_members ADD COLUMN sprite_variant text;
