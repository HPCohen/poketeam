# Pokémon Backend Architecture and Implementation Plan

## 1. Summary and decisions

Build a Rust REST API backed by PostgreSQL, with a separate periodic synchronization worker. Both processes share one codebase and container image.

Confirmed requirements:

- Anonymous browser identity for persistent team ownership.
- Default Pokémon forms across all generations.
- Multiple teams, each containing zero to six distinct Pokémon with persistent ordering.
- Two counter-team modes:
  - **Power mode, the default:** prioritize type coverage, then select the most powerful options by total base stats.
  - **Rival mode:** optimize type coverage while keeping total base stats within ±5% of the source team. Equal team sizes also keep average base stats within that range.
- Change detection includes consumed JSON fields, sprite content, and relevant type-chart changes.
- Alerts describe changes detected within the preceding seven days.

Keep the architecture simple: one Rust application, PostgreSQL, and no message broker or distributed cache.

At implementation start, save this plan and consequential decision rationale under `plans/`, following `AGENTS.md`. Implementation remains paused until requested.

## 2. Architecture and data model

### Application structure

Use Axum/Tokio for HTTP and asynchronous execution, SQLx for PostgreSQL and migrations, Reqwest for upstream requests, Serde for serialization, Tracing for logs, and Utoipa for OpenAPI.

Axum supports Tokio and Tower middleware; SQLx provides explicit SQL, connection pooling, and migrations. [Axum documentation](https://docs.rs/axum/latest/axum/), [SQLx documentation](https://docs.rs/sqlx/latest/sqlx/)

Create one crate under `backend/`, organized by catalog, teams, counters, synchronization, identity, and HTTP. Keep handlers thin and counter scoring independent of HTTP and database access. Inject the upstream client and clock for testing.

Provide these commands:

| Command | Purpose |
|---|---|
| `poketeam serve` | Run the API |
| `poketeam worker` | Run periodic synchronization |
| `poketeam sync` | Perform one synchronization |
| `poketeam migrate` | Apply migrations |

### PostgreSQL model

| Entity | Information |
|---|---|
| `owners` | UUID, hashed browser token, timestamps |
| `pokemon` | Upstream/species IDs, name, ordered types, six base stats, sprite reference, active status |
| `sprite_assets` | Content hash, content type, image bytes |
| `type_effectiveness` | Attacking type, defending type, multiplier |
| `teams` | UUID, owner, name, revision, timestamps |
| `team_members` | Team, Pokémon, position |
| `sync_runs` | Timing, outcome, counts, failure summary |
| `pokemon_changes` | Pokémon, run, detection time, structured before/after changes |
| `catalog_state` | Published revision and last successful refresh |

Use foreign keys, ownership and recent-change indexes, unique member positions, unique Pokémon per team, and positions limited to zero through five.

Update a team and its membership in one transaction. Preserve unavailable Pokémon records for existing teams and history, but exclude them from new selections and generated counters.

Store small sprite assets in PostgreSQL initially. This keeps catalog and image publication transactional without introducing object storage.

### Anonymous ownership

`POST /api/v1/session` creates or resumes an owner using a random token in a persistent HttpOnly, SameSite=Lax cookie. Enable Secure in production, use a rolling one-year lifetime, and store only the token hash.

Require identity for team and notification endpoints. Scope team queries by owner and return `404` for another owner’s team.

Serve frontend and API under one origin. Validate Origin on mutations and require JSON bodies. Clearing the cookie loses access; accounts and cross-device recovery are outside v1.

## 3. API and counter-team behavior

### REST contract

| Endpoint | Behavior |
|---|---|
| `POST /api/v1/session` | Establish anonymous identity |
| `GET /api/v1/pokemon` | Paginated catalog with name search and type filtering |
| `GET /api/v1/pokemon/{id}` | Pokémon details |
| `GET /api/v1/types` | Current types and effectiveness chart |
| `GET /api/v1/sprites/{hash}` | Cached image |
| `GET /api/v1/catalog/status` | Revision and refresh status |
| `GET /api/v1/teams` | Owner’s teams |
| `POST /api/v1/teams` | Create a team |
| `GET /api/v1/teams/{id}` | Team and ordered Pokémon |
| `PUT /api/v1/teams/{id}` | Replace name and ordered membership |
| `DELETE /api/v1/teams/{id}` | Delete a team |
| `POST /api/v1/teams/{id}/counter-team` | Generate an unsaved recommendation |
| `GET /api/v1/notifications` | Recent changes affecting current teams |

Catalog responses include ID, name, ordered types, six named base stats, base-stat total, active status, and nullable local sprite URL. Use stable ID ordering, default page size 60, and maximum 200.

Use immutable caching for hash-addressed sprites and ETags for catalog responses. Expose `/openapi.json` for subsequent TypeScript client generation.

Team creation accepts `{name, pokemon_ids}`, defaulting membership to empty. Updates accept `{name, pokemon_ids, revision}`.

Validate names at 1–100 trimmed characters and membership at zero to six distinct Pokémon. Reject unknown or newly selected inactive Pokémon; existing inactive members may be retained or removed. Increment revisions and return `409` for stale updates.

### Counter-team interface

Request:

```json
{
  "mode": "power"
}
```

Accept `power` or `rival`; omission defaults to `power`.

Return:

- Selected mode and algorithm version.
- Ordered recommended Pokémon.
- Source and counter total/average base stats.
- Percentage stat difference.
- Coverage, matchup score, and shared weaknesses.
- Per-opponent effectiveness explanations.
- Catalog revision.

Generation never saves a team automatically. The frontend saves it through normal team creation.

Both modes return exactly the source team’s size, using distinct active Pokémon. Source species remain eligible. Reject an empty source team.

### Shared type heuristic

For each attacker/defender pair, use the strongest attack from the attacker’s own types, multiplying effectiveness across both defender types.

Calculate:

- **Coverage:** number of opponents that at least one counter hits super effectively.
- **Matchup score:** sum of best effectiveness against each opponent, minus `0.25` per pair of counter Pokémon sharing a weakness to an ordinary attacking type.

Ignore moves, abilities, items, levels, and battle simulation.

### Default power mode

Rank candidate teams lexicographically by:

1. Super-effective coverage.
2. Total base stats.
3. Matchup score.
4. Ascending Pokémon IDs for deterministic ties.

Construct the team greedily, then improve through single-member replacements until no improvement exists or ten passes complete.

This makes raw power the deciding factor after coverage, rather than a minor final tie-breaker.

### Rival mode

Require:

```text
abs(counter_total - source_total) / source_total <= 0.05
```

Rank qualifying teams by:

1. Super-effective coverage.
2. Matchup score.
3. Smallest absolute stat difference.
4. Ascending Pokémon IDs.

Seed the search with the source roster when all members are active, providing an immediately valid stat match. Replace unavailable members deterministically with the nearest-base-stat unused active Pokémon.

Perform single-member improvements while preserving the ±5% constraint, stopping after convergence or ten passes.

If an unavailable-member replacement cannot produce a qualifying seed, return `422 rival_balance_not_found`. Explain that the heuristic did not find a qualifying team; do not imply mathematical impossibility or silently relax the limit.

Run counter search outside asynchronous request execution threads. Both modes are deterministic heuristics, without a global optimality guarantee. Rival mode balances aggregate base stats; it does not guarantee equal battle outcomes.

## 4. Synchronization, notifications, and deployment

### Refresh pipeline

Use PokéAPI species/default varieties, Pokémon details, sprites, and current type relations. Cache resources locally, consistent with its fair-use guidance. [PokéAPI documentation](https://pokeapi.co/docs/v2)

Each scan:

1. Acquires a PostgreSQL advisory lock on a dedicated connection.
2. Discovers default Pokémon across all species.
3. Fetches consumed fields, type relations, and sprite bytes.
4. Normalizes data and hashes image content.
5. Compares the complete candidate snapshot with published data.
6. Atomically publishes catalog changes, assets, events, and a new revision.
7. Records the outcome and releases the lock.

Use four concurrent upstream requests, bounded response sizes, explicit timeouts, and three attempts for transient failures with exponential backoff and jitter. Honor `Retry-After` and restrict upstream URLs and redirects to approved hosts.

Fetch sprite bytes each scan to detect changes at unchanged URLs. Null sprites are valid; failed required downloads abort publication.

Behavior:

- Initial import establishes a baseline without alerts.
- Unchanged scans produce no events.
- Failed scans preserve the previous complete snapshot.
- Only complete discovery can mark records unavailable.
- Type-chart changes create events for Pokémon whose types participate in changed relationships.
- API requests use local data and remain available during upstream outages.

### Notifications

Return events detected within the previous seven days for Pokémon in the owner’s **current** teams.

Include event ID, Pokémon, affected team IDs, detection time, structured differences, and readable descriptions. Deduplicate events across teams and paginate results.

Cover names, types, stats, availability, sprite URL/content, and relevant effectiveness changes. Store sprite hashes rather than image bytes in events.

Retain history for 30 days. No dismissal state in v1; the frontend can poll every 60 seconds and deduplicate by event ID. Timestamps indicate detection time, since the upstream change time is unknown.

### Operations

Run synchronization when the worker starts if missing or overdue, then every 24 hours after success. Retry failures after 15 minutes. Persist scheduling state and prevent concurrent scans.

Provide Docker Compose services for PostgreSQL, API, and worker, with a persistent database volume and separate migration command.

Configure database URL, listen address, public origin, upstream URL, scan interval, and logging through environment variables.

Provide:

- `/health/live` for process health.
- `/health/ready` for database connectivity and initial catalog availability.
- Structured request and synchronization logs.
- Graceful shutdown and rollback.
- Refresh status and documented backup/restoration procedures.

Use consistent JSON errors with machine-readable code, message, and optional details. Apply `400`, `401`, `404`, `409`, `422`, and `503` according to malformed input, identity, ownership/resource lookup, concurrency, domain validation, and catalog availability.

## 5. Implementation and acceptance

### Implementation sequence

1. Save the plan and decision rationale; establish Rust, migrations, configuration, containers, logging, and health endpoints.
2. Implement initial catalog synchronization, sprite caching, catalog APIs, and OpenAPI.
3. Implement anonymous ownership and transactional team CRUD.
4. Implement shared counter scoring, default power mode, rival constraints, explanations, and the generation API.
5. Implement change events, notifications, scheduling, locking, and retention.
6. Complete CI, integration tests, a reproducible demo, and the one-page README.

### Verification

Use disposable PostgreSQL and a local mock upstream; CI must not rely on live PokéAPI.

Test:

- Catalog pagination, filtering, default forms, null sprites, and immutable sprite URLs.
- Team persistence, ordering, membership limits, uniqueness, ownership isolation, deletion, and revision conflicts.
- Type multiplication, immunities, deterministic selection, equal-size output, and unique counters.
- Default mode selection and power ordering after coverage.
- Rival ±5% boundaries, equal average-stat behavior, valid seeding, unavailable members, and explicit search failure.
- Initial-import silence, unchanged scans, all consumed-field changes, sprite changes at unchanged URLs, and type-chart impact.
- Seven-day boundaries, current-team membership, and event deduplication.
- Upstream failures, atomic rollback, overlapping workers, and restart scheduling.

Provide a demo that imports a mock baseline, creates a team, changes a selected Pokémon’s stat and sprite, runs the normal sync path, and shows the resulting notification and image URL. Repeat synchronization to demonstrate deduplication and advance an injected clock to demonstrate expiry.

CI runs formatting checks, Clippy, unit/integration tests, OpenAPI generation checks, and a container build.

The deliverable is a verified backend and documented React/TypeScript integration contract. Frontend implementation and production hosting provisioning follow separately.
