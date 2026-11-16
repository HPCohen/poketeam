# Operations

## Local commands

Use `docker compose up -d db`, `docker compose build`, `docker compose run --rm migrate`, then `docker compose up -d api worker`. Run migrations before starting new API/worker versions. Migrations are embedded in the binary. If Docker Hub transfers are slow, the build accepts an equivalent Rust image mirror with `docker compose build --build-arg RUST_IMAGE=mirror.gcr.io/library/rust:1.99.1-slim-bookworm`. For a manual refresh use `docker compose run --rm worker sync` (overrides the worker command). `poketeam serve`, `worker`, `sync`, `migrate`, and `openapi` are also available via `cargo run --manifest-path backend/Cargo.toml -- <command>`. `openapi` requires no database configuration. No live upstream calls are made in tests or the demo.

The development database binds to localhost:5432 and the API to localhost:3000. Compose keeps a persistent named PostgreSQL volume. `docker compose down` preserves it. The fixture-based demonstration requires the database and Rust, then `scripts/demo.sh`; it creates isolated test databases, prints actual API notification payloads and image URLs, asserts sprite bytes changed, repeats sync, and advances the injected clock past expiry. SQLx removes successful test databases automatically; failed test databases may be retained for inspection.

## Configuration

Copy values from `.env.example` into your shell environment for native runs; the binary does not load dotenv files. Compose reads `.env` for interpolation. `DATABASE_URL` is required for all commands except `openapi`. `LISTEN_ADDR` defaults to `0.0.0.0:3000`. `PUBLIC_ORIGIN` defaults to `http://localhost:3000`; HTTPS enables Secure cookies. Terminate TLS at the same-origin reverse proxy in production and use private database networking/credentials. Frontend assets are served separately under that origin. The API exposes no cross-origin access policy.

`UPSTREAM_URL` defaults to `https://pokeapi.co/api/v2/` and must end with `/`. `UPSTREAM_ALLOWED_HOSTS` defaults to `pokeapi.co,raw.githubusercontent.com`; this applies to discovered resource URLs, sprites, and every redirect hop. `ALLOW_INSECURE_UPSTREAM=true` is for explicit local mock HTTP use. `SCAN_INTERVAL_SECONDS` defaults to `86400` and must be positive. `RUST_LOG` configures structured tracing.

Sync saves the complete type chart first, then follows species/default-variety resources and saves each Pokémon as it arrives. Each Pokémon, sprite asset, change event, revision, and progress count commit in one short transaction before the next fetch. No transaction spans network requests. JSON responses decode directly through reqwest into Serde models in `backend/src/model/pokeapi.rs`. Every request has a 30-second timeout and 10-second connect timeout. Requests fail the pass immediately; the worker retries the whole pass after 15 minutes. Reqwest follows at most five redirects under the configured host/scheme restrictions. Sprite downloads check their advertised and downloaded size against 4 MiB; JSON uses the built-in decoder without a custom streaming buffer. Only PNG, GIF, JPEG, and WebP sprites are accepted. A failed JSON or sprite download stops the pass while retaining earlier committed Pokémon. Missing/default-replaced Pokémon are marked unavailable only after every resource has been processed. Null sprites are valid. The normalized catalog and sprites are cached in PostgreSQL; daily scans intentionally refetch sprite bytes to detect content changes at unchanged URLs.

## Health and scheduling

`/health/live` reports the running HTTP process. `/health/ready` checks database connectivity and catalog readiness. It returns 503 until the complete type chart and at least one Pokémon have committed; a partial initial import can then be browsed. `/api/v1/catalog/status` reports catalog revision, last successful refresh, and the latest run’s outcome/counts/failure summary. Run status reports `pokemon_count` as processed records commit, including on failure. `last_success_at` changes only after a complete scan; a ready catalog can coexist with a failed or running latest scan. Revisions advance on visible committed changes. Inspect `docker compose logs api worker` for structured HTTP and sync logs. Request logs omit cookie headers and bodies. Sync emits a `Pokémon saved` progress entry after each record commits. Initial-import notifications remain silent until the first fully successful pass; later committed changes generate notifications even when the pass eventually fails.

The worker checks persisted run state at startup, waits until due, scans every 24 hours after success, and retries failures after 15 minutes. A PostgreSQL session advisory lock lives on a dedicated connection. Overlapping attempts skip, without fetching upstream data, and recheck after a minute. A crashed `running` entry triggers a restart attempt; after acquiring the lock, the next worker marks unfinished runs abandoned. Ctrl-C/SIGTERM stop waiting or cancel a scan, closing the dedicated connection and rolling back only the current open transaction. Earlier committed records remain saved. Final availability reconciliation, pruning, last-success timestamp, and success status commit together. Container restarts recover from database startup/outages.

Change history and unreferenced run records older than 30 days are pruned on successful scans. Pokémon and hash-addressed assets are retained for current teams, history, and immutable image URLs. There is deliberately no automatic asset deletion that could invalidate a previously issued immutable URL. Monitor database growth before introducing an asset retention/storage policy; object storage is a future migration, already identified in the approved plan.

## Backup and restoration

PostgreSQL contains all authoritative state, including hashed anonymous credentials, team memberships, sprites, and scheduling history. Back up the entire database with a version-compatible PostgreSQL client and protect backups as private data:

```sh
docker compose exec -T db pg_dump -U poketeam -d poketeam -Fc > poketeam.dump
```

Test restoration in a separate database before relying on a backup:

```sh
docker compose exec -T db createdb -U poketeam poketeam_restore
docker compose exec -T db pg_restore -U poketeam -d poketeam_restore --exit-on-error < poketeam.dump
docker compose exec -T db psql -U poketeam -d poketeam_restore -c 'SELECT revision,last_success_at FROM catalog_state;'
```

To recover production, stop API and worker, provision an empty database, restore the complete dump, point `DATABASE_URL` to it, run the matching release’s migrations, and restart both processes. Preserve the public origin/cookie name so existing browsers retain ownership. The raw tokens exist only in browser cookies, so restoring older backups cannot recover identities/teams created after the backup. Never restore over a running application database. Retain previous release images and database backups for rollback; prefer restoring a matching schema/database rather than inventing destructive down migrations.

## AWS starter deployment

See [aws/README.md](../aws/README.md) for the separate low-cost AWS stack and CLI workflow. It uses a production-specific Compose file; it does not expose the development database port or reuse its fixed development password. No AWS resources are created as part of local backend checks.

## Capture a manual sync log

With the normal backend environment set, run `scripts/sync-with-log.sh` from the repository. It builds/runs one sync and captures both stdout and stderr to a unique timestamped `logs/sync-*.log` while displaying them in the terminal. The script prints the file path before starting and returns a failing status if the sync fails. Set `SYNC_LOG_DIR` to change the destination. Logs are ignored by Git.

Upstream request failures include the requested URL and full error chain. Final request failures and JSON decoding failures also include URL context in CLI stderr. The database's `failure_summary` remains a short summary. For the current native development setup:

```sh
DATABASE_URL=postgres://poketeam:poketeam@127.0.0.1:5432/poketeam \
PUBLIC_ORIGIN=http://127.0.0.1:5173 \
scripts/sync-with-log.sh
```

## Minimal reqwest diagnostic

Run the standalone probe without a database or application environment:

```sh
cargo run --locked --manifest-path backend/Cargo.toml --bin reqwest-probe -- \
  https://pokeapi.co/api/v2/pokemon/4/
```

With no URL arguments it uses that endpoint. Supply multiple URLs (including repeated URLs) to fetch sequentially with one shared client and exercise connection reuse. Each response is decoded using reqwest's built-in JSON decoder, retained in memory, and printed in full to stdout. Stderr records URL, status, HTTP version, final URL, advertised length, timings for headers and complete JSON, and full failure context. A failure exits nonzero.

The probe uses the sync client's reqwest/TLS features, User-Agent, 30-second total timeout, and 10-second connect timeout. It uses normal reqwest redirects and omits application configuration, model conversion, sprite downloads, and database access. To capture everything, redirect `> logs/reqwest-probe.log 2>&1` after creating `logs/`. Existing Cargo run commands still default to the application binary.

## Sprite variant metadata

Migration `0002_sprite_variants.sql` adds `pokemon.sprite_variants` and `team_members.sprite_variant`. Existing records are backfilled with their current default image, and existing teams keep the default. Run the normal migration command before updating the API, then run a sync to populate all available variant URLs. Sync continues to download/cache only the default sprite; other images (including SVG and GIF) load from their upstream URLs in the browser when chosen. This preserves incremental sync and avoids storing every game/artwork variant in PostgreSQL.

## Catalog filter metadata

Run migration `0003_catalog_filters.sql` before updating the API/worker, then run a normal sync to populate species introduction generation and starter/Legendary/Mythical flags. Existing rows remain readable with null generation and false flags until their incremental refresh commits. Generation/category filtering is fully populated after the complete pass. No additional upstream requests are needed: the existing species response contains generation and Legendary/Mythical status. Starter families are explicitly classified in `backend/src/model/pokeapi.rs` and should be reviewed when new main first-partner lines or Eevee evolutions appear.
