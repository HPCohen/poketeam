# React/TypeScript frontend implementation

Implements the approved frontend plan supplied for this task. Scope: repository source and a static build in `frontend/dist`; production deployment and backend changes are excluded.

## Decisions

The approved React/TypeScript/Vite stack uses React Router for URL navigation and TanStack Query for cancellable server reads, invalidation, and visible-page polling. Components are grouped by feature without another package or design-system abstraction. Plain CSS provides a light Pokédex appearance, red accents, type badges, responsive grids, visible focus, and keyboard controls. Sprites come from the backend; missing images show a labeled fallback. No external image or font dependency is required.

The editor owns its draft above nested Pokémon details. Drafts live in memory only. Explicit saves carry the loaded revision, and conflicts keep the draft until the user explicitly discards/reloads. Router navigation and browser unload warn about unsaved work. Existing inactive members remain selectable for retention, ordering, or removal; catalog additions require active members, distinct IDs, and capacity below six. New teams may have zero members. Names follow the backend's trimmed 1–100 Unicode-character rule.

A shared fetch helper handles JSON mutations (including DELETE `{}`), cancellation, same-origin cookies, structured errors, and 204 bodies. The pinned OpenAPI TypeScript generator produces committed types, checked for drift using a temporary output. Endpoint functions use these types. Private queries and editor state are cleared on 401, with no automatic session replacement or mutation replay. Public catalog access remains available without identity. Session creation requires an explicit Get started action. No credentials or drafts are stored in localStorage.

Counter generation targets saved nonempty teams, uses explicit Power/Rival modes, and produces only a preview. The separate save form requests a name and creates a new team. Results expose stat totals/averages, percentage difference, coverage, matchup explanations, and shared weaknesses. Rival search failure explicitly describes heuristic failure within the ±5% rule.

Notifications use URL pagination, replace each page on refresh, deduplicate event IDs, and poll every 60 seconds while visible. Team mutations invalidate teams, individual team queries, and notifications. The backend remains authoritative for the seven-day window and ownership.

These are reversible implementation choices within the approved architecture. No new irreversible data, hosting, or architecture decision is introduced.

## Files and integration

- `frontend/`: application, pinned npm dependencies/lockfile, generated contract, unit/component tests, Playwright browser tests, smoke script, and setup documentation.
- `.github/workflows/frontend.yml`: reproducible dependency installation, API drift, TypeScript, Vitest, static build, and desktop/mobile Chromium checks.
- Root README, AWS frontend upload instructions, and API documentation: setup, exact Vite browser origin, explicit session initialization, and frontend/API integration.
- `.gitignore`: generated frontend build, dependencies, and test output.

## Verification

Executed locally on 2026-11-15:

- `npm run generate:api` and `npm run check:api`: pass; generated types match the backend contract.
- `npm run typecheck`: pass.
- `npm test`: 14 component/API tests pass, covering explicit sessions, 401 cleanup/no replay, draft navigation/close warnings, validation, revision conflicts, retained unavailable members, ordered save payloads, preview-only counters, Rival failures, and notification replacement/deduplication.
- `npm run build`: pass; deployable static output in `frontend/dist`, approximately 108 KB gzip JavaScript and 2.7 KB gzip CSS.
- Live smoke against the existing Rust backend, both directly and through Vite: pass for explicit session cookies, HttpOnly, exact Origin rejection, empty-team CRUD, loaded-revision conflict, and empty 204 deletion. The initially empty development database required the existing migration. Before sync, its catalog was uninitialized, so live catalog/counter checks were skipped.

The local host runs Node 26. Network timeouts and stalled optional native installation required retrying cached npm installation with `--ignore-scripts`; the binary esbuild package builds successfully. CI uses Node 22 on Linux. Playwright’s completed Chromium archive stalled in the JavaScript extractor, so the downloaded archive was unpacked with system `unzip` in `/tmp/poketeam-browsers` for local testing. No application behavior was bypassed. Browser verification caught and fixed initial focus stealing the skip link; subsequent SPA navigation still moves focus to main content.

Browser tests use mocked HTTP responses to keep CI independent of upstream Pokémon availability. The live smoke script creates and removes only its own uniquely named team.

## Local review follow-up

The user requested a backend sync after tests pass and asked that the frontend and API remain running for inspection. Vite runs at `http://127.0.0.1:5173`, with the backend at `http://127.0.0.1:3000` and its `PUBLIC_ORIGIN` matching Vite exactly. PostgreSQL remains the existing Compose development service. Final outcomes are recorded below.

- Final Playwright run: **4/4 pass** (complete workflow and keyboard/public-draft workflow on desktop Chromium and mobile Chromium). Tests include welcome/session creation, searched detail/add flow, roster ordering/save/reload, counter preview/save, editing/deletion, notification refresh/deduplication, and direct links.
- The requested first live sync failed with an upstream connection reset at `pokemon-species/4`; the backend did not publish a partial catalog. The retry also failed with an upstream response-body timeout. No partial catalog was published; live Pokémon/counter compatibility remains unverified. Both services are left running, with the frontend correctly showing catalog initialization until a successful sync.
- Final live smoke after both sync attempts: **pass** for frontend HTTP 200, API liveness 200 through the proxy, cookie/Origin/session handling, team CRUD/revision conflict, and empty 204 deletion. Catalog remains uninitialized because of upstream failures. API and Vite are intentionally left running for the requested review.

Follow-up: the [incremental sync implementation](incremental-sync.md) completed a live pass over all 1,025 Pokémon. The previously blocked live catalog/counter smoke checks now pass through Vite, alongside session, Origin, team CRUD and revision handling.
