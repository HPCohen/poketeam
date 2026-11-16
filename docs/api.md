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
| GET `/pokemon` | `limit=60` (1–200), `offset=0`, optional `search` (literal case-insensitive substring), `type` (exact type name) → `{pokemon,total,limit,offset,catalog_revision}`; active only, ascending IDs |
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
