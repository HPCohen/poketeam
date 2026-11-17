# React/TypeScript integration contract

The canonical machine-readable contract is [backend/openapi.json](../backend/openapi.json), also served at `/openapi.json`. Regenerate with `cargo run --manifest-path backend/Cargo.toml -- openapi > backend/openapi.json`. The frontend pins `openapi-typescript` and commits `frontend/src/api/generated.d.ts`. Run `cd frontend && npm run generate:api` after contract changes; `npm run check:api` detects drift. See [frontend setup](../frontend/README.md).

Serve the frontend and API under one origin (a Vite proxy is suitable for development). Set `PUBLIC_ORIGIN` to the browser’s exact origin, without a path or trailing slash. No CORS configuration is needed. Cookies are HttpOnly and should never be read by React. Send `credentials: 'same-origin'` (fetch’s default); the browser supplies `Origin` on mutations. Command-line clients must send it explicitly.

```typescript
async function api<T>(path: string, method = 'GET', body?: unknown): Promise<T> {
  const response = await fetch(`/api/v1${path}`, {
    method,
    credentials: 'same-origin',
    headers: body === undefined ? {} : { 'Content-Type': 'application/json' },
    body: body === undefined ? undefined : JSON.stringify(body),
  });
  if (!response.ok) throw await response.json(); // { code, message, details? }
  return response.status === 204 ? undefined as T : response.json();
}
await api('/session', 'POST', {}); // Explicit Get started only; initial load probes GET /teams.
const team = await api<{id: string; revision: number}>(
  '/teams', 'POST', { name: 'Favorites', pokemon_ids: [25, 6] },
);
await api(`/teams/${team.id}`, 'PUT', {
  name: 'Favorites', pokemon_ids: [6, 25], revision: team.revision,
});
const recommendation = await api(`/teams/${team.id}/counter-team`, 'POST', {});
// Save a recommendation explicitly via POST /teams with its ordered Pokémon IDs.
await api(`/teams/${team.id}`, 'DELETE', {});
```

| Method/path (relative to `/api/v1`) | Input and result |
| --- | --- |
| POST `/session` | `{}` → `{owner_id}` and rolling one-year cookie |
| GET `/pokemon` | `limit=60` (1–200), `offset=0`, optional `search` (literal case-insensitive substring), `type` (legacy exact type), `types`, `type_match`, `generation`, `category` (see below) → `{pokemon,total,limit,offset,catalog_revision,generations}`; active only, ascending IDs |
| GET `/pokemon/{id}` | Pokémon, including retained inactive records |
| GET `/types` | `{types,effectiveness,catalog_revision}`; effectiveness maps attacker → defender → multiplier |
| GET `/sprites/{hash}` | Image bytes with `public, max-age=31536000, immutable` |
| GET `/catalog/status` | `{revision,last_success_at,latest_run}`; revision 0 means no usable catalog yet; partial imports become ready after the type chart and at least one Pokémon commit |
| GET `/teams` | Owner’s teams, ordered by creation time then ID |
| POST `/teams` | `{name,pokemon_ids?:number[]}` → team, status 201 |
| GET `/teams/{id}` | Team with ordered Pokémon |
| PUT `/teams/{id}` | `{name,pokemon_ids,revision}` → updated team with incremented revision |
| DELETE `/teams/{id}` | `{}` → status 204 |
| POST `/teams/{id}/counter-team` | `{mode?:'power'|'rival'}` → unsaved recommendation |
| GET `/notifications` | `limit=60` (1–200), `offset=0` → `{notifications,total,limit,offset}`, newest detection/event ID first |

Pokémon expose `id` (default variety’s Pokémon ID), `species_id`, `name`, ordered `types`, `stats` (`hp`, `attack`, `defense`, `special_attack`, `special_defense`, `speed`), `base_stat_total`, `active`, and nullable local `sprite_url`. IDs differ from species IDs for some default forms. Team responses expose `id`, trimmed `name`, `revision`, `created_at`, `updated_at`, and ordered `pokemon`. Times are RFC 3339 UTC. PostgreSQL bigint revisions/event IDs serialize as JSON numbers; practical v1 values fit JavaScript’s safe integer range.

Counter results expose `mode`, `algorithm_version`, ordered `pokemon`, `source_total`, `source_average`, `counter_total`, `counter_average`, signed `percentage_stat_difference`, `coverage`, `matchup_score`, `shared_weaknesses`, per-opponent `explanations`, and `catalog_revision`. Counter output sorts by ID for deterministic presentation. Equal sizes make the percentage differences in total and average stats equal. Source species remain eligible. Rival mode never relaxes the ±5% constraint.

Notifications expose `id`, current `pokemon`, sorted `affected_team_ids`, `detected_at`, `differences`, and readable `descriptions`. Ordinary differences use `{before,after}`; `type_effectiveness` contains `{attacking,defending,before,after}` relationships. A newly discovered Pokémon has an `availability` before/after record. Sprite differences contain hashes/source URLs, never bytes. Detection time is when the worker observed the change, not the unknown upstream edit time. Poll every 60 seconds and deduplicate by event ID. Membership changes can alter which historical events appear, so replace the visible window on refresh. Fetch additional pages when needed.

Catalog, details, and type-chart responses support ETags and `If-None-Match` with 304 responses. The revision changes whenever a Pokémon or type-chart update commits during sync. Partial catalog updates remain visible if a later request fails. Team and notification data should not be cached across identities. Persist the cookie in the browser, not owner IDs or tokens in localStorage. Use the same selected IDs and revision when editing; on `409 revision_conflict`, reload before retrying. Newly selected inactive members are rejected, but existing inactive members may be retained/reordered or removed.

Errors use `{code,message,details?}`: `400` malformed JSON/query/path, invalid Origin, or missing JSON content type; `401 identity_required`; `404 not_found` (including another owner’s team); `409 revision_conflict`; `422 validation_error`, `empty_source_team`, `insufficient_active_pokemon`, or `rival_balance_not_found`; `503 catalog_unavailable`, `database_unavailable`, or `counter_busy`. Internal counter failures use `500 counter_failed`. Rival failure explicitly indicates a heuristic search failure. Reject an empty team before offering generation. Every mutation, including session creation and deletion, requires a JSON object and the same-origin header.

For local frontend development, set `PUBLIC_ORIGIN=http://127.0.0.1:5173` and open that exact Vite URL. Proxy requests retain the browser Origin for backend validation. The shared typed frontend helper is `frontend/src/api/client.ts`. It clears private data on 401 and never replays mutations; a 409 preserves editor drafts until explicit discard/reload.

Sync status counts processed Pokémon as they commit, including for running/failed scans. `last_success_at` records the last fully completed scan. Failed scans retain earlier committed rows and alerts; missing Pokémon become unavailable only after a complete scan. Initial-import alerts remain silent until the first fully completed scan.

## Sprite versions

Each Pokémon now includes `sprite_variants`, a map from stable upstream paths to image URLs, and nullable `selected_sprite` (set for saved team members). Paths include `front_shiny`, `back_default`, `other/official-artwork/front_default`, and `versions/generation-i/red-blue/back_default`. Only non-null image URLs are offered. Nested SVG artwork and animated GIF variants are included. These are appearances for the existing catalog Pokémon; they do not change its form/stats.

The default image stays cached at the immutable local `sprite_url`; its `front_default` map entry points there. Other map entries reference the upstream image and load directly in the browser when selected. Sync records the URLs and does not eagerly download every image. Default sprite byte changes are detected as before; additional variant content is not downloaded/hashed, so those images produce change alerts when their metadata URLs change.

Team creation/update accepts optional `sprite_selections`, keyed by Pokémon ID string, with each value taken from that Pokémon's map. For example, `{ "name": "Shiny favorites", "pokemon_ids": [25,6], "sprite_selections": {"25":"front_shiny"} }`. On update also send the loaded `revision`. Omitted entries preserve a retained member's existing choice; explicit `front_default` resets it. Selections must belong to the submitted roster and exist in its metadata, except an existing saved selection can be retained if upstream removes it. Each team stores its own choice independently. The UI falls back to the default image for missing/broken variants; appearance changes participate in the same explicit Save/conflict flow as roster edits.

## Catalog filters

`GET /api/v1/pokemon?types=fire,flying&type_match=all&generation=1&category=starter` returns Charizard. `types` accepts comma-separated type names (case-insensitive, trimmed and deduplicated). `type_match=all` is the default and requires every selected type, with at most two distinct types. `type_match=any` matches at least one selected type. The legacy `type=fire` still works; `types` takes precedence when supplied.

`generation` is a positive introduction-generation number from species metadata, independent of sprite version. `category` is one of `starter`, `legendary`, or `mythical`; Legendary and Mythical remain separate. Starter families include the nine generations' main first-partner evolution lines, Pichu/Pikachu/Raichu, and Eevee with all its evolutions. This explicit species list needs updating if new starter families are introduced. Name, type, generation and category filters combine with AND. Pagination applies after filtering, and `total` counts all matches. `generations` contains all available imported introduction generations, independent of filters.

Pokémon include nullable `generation` and boolean `is_starter`, `is_legendary`, `is_mythical`. Existing rows have unknown generation and false flags until refreshed. Introduction generation and Legendary/Mythical status come from [PokéAPI species metadata](https://pokeapi.co/docs/v2#pokemon-species); starter membership is defined by the application. Invalid enum/integer syntax returns 400; unknown types, nonpositive generations and more than two types in All mode return 422.

## Authentication-provider foundation

`owner_id` identifies a stable application user, independent of a browser credential. The backend stores each device credential in `owner_sessions`; multiple sessions can authorize the same owner and teams, with independent inactivity expiry and revocation. `POST /session` remains anonymous Get started and returns the same `{owner_id}` response. Existing valid cookies are migrated without changing ownership. No public endpoint accepts an owner ID as proof of identity.

The trusted backend extension points are `identity::sessions::issue` and owner-scoped `revoke`. `issue` accepts an owner already established by server-side authentication and returns cookie material through `cookie_header`; its raw bearer token is private and is never serialized. A future provider callback must verify the login proof, resolve its provider identity to an application owner, then issue a device session. Provider selection, callbacks, identity bindings, account linking/anonymous-team merging, login/logout UI and authenticated-session policies are deferred. Anonymous sessions still rely on the current browser cookie; cross-device sign-in is not enabled yet. See [the design and deferred decisions](../plans/auth-provider-foundation.md).

## Manual notification demo

`PATCH /api/v1/demo/pokemon/{id}` is an opt-in operator endpoint. It returns 404 unless the backend has a nonempty `DEMO_UPDATE_TOKEN`. Supply `Authorization: Bearer <server token>`, the exact configured `Origin`, and JSON such as `{"stats":{"hp":50,"speed":95}}`. The token must be at least 32 printable non-space ASCII characters. Browser cookies alone never authorize this global catalog update. Keep the token server/operator-only.

Only the six base stats can be changed (`hp`, `attack`, `defense`, `special_attack`, `special_defense`, `speed`). Supply at least one integer in 1–1000; unknown/malformed fields return 400, empty/out-of-range changes 422, a missing/wrong token 401, and an unknown Pokémon 404. The initial full catalog import must have completed (otherwise 503). Existing exact-Origin/JSON mutation requirements also apply.

The response is `{pokemon,catalog_revision,change_id}`. A real change updates the catalog/revision and inserts a normal before/after notification in the same transaction. An identical submission returns `change_id:null` and leaves revision/history unchanged. Alerts appear only to owners whose current teams contain that Pokémon, using existing deduplication and seven-day visibility rules. Edits are global, not per-team overrides. A later normal sync restores upstream values and can generate another notification. Manual edits do not change sync scheduling, status or counts.
