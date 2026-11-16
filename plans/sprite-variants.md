# Sprite variants and per-team member choices

The user requested exposing every available sprite version instead of the single `front_default` image currently imported, and explicitly chose to save the selection separately for each Pokémon in each team.

## Scope and decisions

Expose all non-null sprite URLs supplied for each existing catalog Pokémon: front/back, shiny, gender variants, game generations/versions, artwork, SVG images and animated GIFs. Use stable upstream JSON paths as keys, such as `front_shiny` or `versions/generation-i/red-blue/back_default`. This changes appearance only; the existing default-variety catalog and battle/stat model remain intact.

The user chose to store URLs and load the selected image on demand. Keep the existing default sprite's local content-addressed cache; additional variants use upstream HTTPS image URLs directly in browser image elements. This includes GIF/SVG variants without expanding server image ingestion or importing the entire collection. A sampled Pokémon has 149 non-null variants, illustrating the avoided bandwidth/storage cost.

Migration `0002_sprite_variants.sql` adds a JSON map to catalog rows, backfilled with the existing default URL, and a nullable selected key on each team member. Keys remain stable across upstream URL changes. `sprite_selections` is an optional team mutation map keyed by Pokémon ID: omitted entries preserve existing choices; explicit `front_default` resets a member to default. Selections for other members or unknown variants are rejected. Existing missing keys may be retained and shown with a default-image fallback. No original migration is rewritten.

The per-team selection must remain a draft until Save, survive save/reload/reordering, and be independent between teams. Older clients must retain selections when omitted. Removed sprite metadata must fall back clearly to the normal sprite without invalidating an existing saved team. Keep revision conflicts, unavailable Pokémon retention, and ownership validation.

## Validation

Cover nested/null/gender/artwork/animation metadata, selection validation and persistence, backward-compatible updates, cross-team independence, missing variant fallback, draft dirty state and navigation, selector previews, and a complete browser save/reload flow. Regenerate API types and run backend/frontend checks. Record migration, catalog refresh, and live verification outcomes below.

## Checks

The backend's 26 tests, 18 frontend component/API tests, and six desktop/mobile browser workflows pass. Coverage includes variant metadata extraction (nested games, gender, SVG, animation, nulls), validation, saved-team independence, backward-compatible updates, removed choices, ownership/revision checks, draft navigation, selected previews, saved/reordered/reloaded appearances, and image fallback. Clippy, TypeScript, generated-type drift and production build pass. The default cache download-count test confirms variants are not eagerly downloaded. Local migration/catalog refresh and live verification follow below.

## Local rollout and live verification

Applied migration `0002_sprite_variants.sql` to the local development database, restarted the API, and refreshed all 1,025 catalog entries successfully. The sync log is `logs/sync-20261116T030257Z-59973.log`; each record gained its full variant URL map while only default images were fetched. Bulbasaur exposes 149 non-null entries.

Live API checks through Vite passed for shiny/artwork/game/SVG/GIF metadata, two teams with independent selections, reordered members, backward-compatible omission retention, stale-revision rejection, and explicit reset to default. Temporary test teams were deleted. A real Chromium session loaded shiny PNG, SVG Dream World art, and animated GIF variants successfully; a local visual preview confirmed the selector fits the detail layout. All 50 automated tests passed (26 backend, 18 frontend component/API, six desktop/mobile browser workflows), and frontend type/drift/build checks pass after the final label update. Additional filtering work requested during the rollout continues in its own plan.
