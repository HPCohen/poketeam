# Advanced Pokémon search filters

The user requested multi-type combinations, introduction generation, starter, and legendary filters. They chose full starter evolution families (including Pikachu and Eevee) and separate Legendary/Mythical groups.

## Design

Add additive species metadata columns for introduced generation, starter-family membership, legendary status and mythical status. Read generation/is_legendary/is_mythical from PokéAPI species responses during the existing per-Pokémon sync. Starter families require a small explicit species-ID classification because the API has no starter flag. Document the base first-partner lines and Pikachu/Eevee relatives; future first-partner families need an explicit list update. Keep existing IDs, sprite choices, stats, and incremental writes compatible.

Extend the existing catalog endpoint with `types` (comma-separated, normalized/deduplicated), `type_match=all|any`, `generation` (positive integer), and `category=starter|legendary|mythical`. Preserve the legacy single `type` parameter. All matches require every selected type (up to two); Any matches at least one selected type. Generation means the species' introduction, not the sprite/game version. Name search, type selection, generation and category combine with AND. Return available generations in catalog pages so the UI follows imported metadata rather than hardcoding available generations.

The explorer and team-builder selector share controls and URL state: type checkbox badges, All/Any selector, generation and group dropdowns, and Clear filters. Filter changes reset pagination while pagination retains all filters. New metadata also appears on Pokémon cards/details. No new endpoints or hosting resources are required. The migration is additive; metadata is populated by a normal sync before final live verification.

## Verification

Test type conjunction/disjunction, duplicate/legacy input, invalid filters, generation/category/name combinations, pagination/totals, introduction metadata import and starter classification, URL/back-navigation and reset behavior, and both explorer/team-builder use. Keep sprite selection tests passing. Regenerate OpenAPI/types, run backend/frontend tests, Clippy/type/drift/build checks and desktop/mobile browser workflows, then apply the local migration, refresh metadata and verify real filter results.

## Results

Backend checks passed: 28 unit/integration tests, Clippy, formatting and OpenAPI drift. Frontend checks cover 20 component/API tests and eight desktop/mobile browser workflows, including combined filters, URL restoration, pagination and team building. A mobile test exposed rapid Clear/select actions reusing stale URL parameters; pending parameters now update synchronously before navigation so successive controls preserve each other's changes. Results remain visible while the next query loads.

Live migration, metadata refresh and real-data verification follow the automated checks. Sprite choices remain independent of catalog filters and use the existing team-member save flow.

## Local rollout

Applied migration `0003_catalog_filters.sql`, restarted the native API, and completed a normal sync of all 1,025 Pokémon with 1,025 metadata-change events. Full output is in `logs/sync-20261116T132919Z-82465.log`; no requests failed. The running API uses `logs/api-catalog-filters.log`, and Vite remains available at http://127.0.0.1:5173.

Live verification passed for all nine introduction generations and 93 starter-family species, including Pichu/Pikachu/Raichu and every Eevee evolution. Combined Fire/Flying + generation I returns Charizard for starters and Moltres for Legendary; Psychic + generation I separates Mewtwo (Legendary) from Mew (Mythical). Electric + generation II + starters returns Pichu. Legacy single-type queries, All/Any, pagination/totals, validation and service readiness passed against the real database. Real desktop/mobile browsers verified combined controls and results with no horizontal overflow; frontend session/team CRUD/revision smoke checks passed and cleaned up their temporary team. The previously failing advanced-filter browser workflow passed three additional runs on each viewport. All 56 automated tests and type/drift/build checks passed; changes remain uncommitted for review.
