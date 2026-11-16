# Poketeam

A Rust/Axum REST API and periodic worker backed by PostgreSQL. Both commands use the same image. The catalog contains species’ default varieties across all generations; API requests read only local data. The React/TypeScript frontend lives in `frontend/`; production deployment remains a separate action.

```sh
docker compose up -d db
docker compose build
docker compose run --rm migrate
docker compose up -d api worker
# Ready once the worker saves the type chart and its first Pokémon:
curl http://localhost:3000/health/ready
```

**Design.** Explicit SQL, a single crate, and no broker or distributed cache keep the architecture small. Each sync follows paginated resource lists and fetches details sequentially with reqwest’s JSON decoder. It saves the type chart first, then commits each Pokémon with its sprite and change event before fetching the next. An advisory lock prevents overlapping scans. Failures retain already committed records; missing Pokémon are marked unavailable only after a complete scan; the worker retries after 15 minutes and normally refreshes 24 hours after success. PostgreSQL stores small sprites so each image and Pokémon row commit together.

**Ownership and teams.** `POST /api/v1/session` with `{}` establishes a persistent anonymous HttpOnly cookie. Same-origin JSON mutations require an exact `Origin` match. HTTPS origins enable Secure cookies. Teams have zero to six distinct, ordered members and optimistic revisions; ownership is enforced in SQL. Clearing the cookie loses access. Accounts and cross-device recovery are outside v1.

**Counter teams.** Power mode prioritizes coverage, then total base stats, then matchup score. Rival mode preserves total stats within ±5% and prioritizes coverage and matchup score. Both are deterministic greedy/local-search heuristics capped at ten improvement passes. Own-type attacks multiply effectiveness across defender types; shared weaknesses incur a penalty. Moves, abilities, items, levels, and battle outcomes are outside the model. Rival search failure means the heuristic found no qualifying seed, not that none exists.

**Change alerts.** Initial import is silent. Consumed fields, availability, sprite source/content, and relevant type-chart relationships produce structured events with detection timestamps. Notifications deduplicate events across the owner’s current teams for an inclusive seven-day window. History is pruned after 30 days during successful scans. Existing teams retain unavailable members; new selections and counters use active Pokémon.

**Verification.** Install Rust 1.99.1, start the database above, then run:

```sh
export DATABASE_URL=postgres://poketeam:poketeam@localhost:5432/poketeam
cargo test --locked --manifest-path backend/Cargo.toml
scripts/demo.sh             # Local mock baseline → stat/sprite change → alert → dedup → expiry
scripts/check-openapi.sh
```

Tests use isolated SQLx databases and a local HTTP fixture; CI never calls live PokéAPI. CI also checks formatting, Clippy, OpenAPI drift, and container builds. See [captured demo](docs/demo.md), [API/TypeScript integration](docs/api.md), [operations and backup/restore](docs/operations.md), [approved plan](plans/approved-plan.md), and [implementation/verification record](plans/implementation.md). Configuration is documented in [.env.example](.env.example); Compose credentials are for local development.

For AWS hosting, see [AWS CLI deployment scripts](aws/README.md) for a single-host deployment, Free Tier checks, HTTPS, backups, future frontend uploads, and teardown.

## Frontend

Run `cd frontend && npm ci && npm run dev`, then open **http://127.0.0.1:5173**. Set the backend’s `PUBLIC_ORIGIN=http://127.0.0.1:5173` to match that exact browser origin. Vite proxies API requests to port 3000. `npm run build` produces `frontend/dist` for the existing AWS deployment scripts. See [frontend setup and checks](frontend/README.md) and [implementation record](plans/frontend.md).
