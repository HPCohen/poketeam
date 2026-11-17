# Manual catalog updates for notification demos

The user requested an endpoint to manually update an entry and demonstrate update notifications. Add an operator-only `PATCH /api/v1/demo/pokemon/{id}` accepting partial changes to the six existing base stats, for example `{"stats":{"hp":50}}`. At least one stat is required; values use the upstream model's 1–1000 range. Reject unknown/malformed fields. Other Pokémon metadata stays intact.

## Behavior and access

The endpoint is disabled (404) unless a server-only `DEMO_UPDATE_TOKEN` is configured. Require `Authorization: Bearer <token>` plus the existing exact Origin and JSON mutation policy. Browser ownership cookies do not grant global catalog-edit permissions. Validate a configured token as at least 32 printable non-space ASCII characters; compare hashes with the existing subtle constant-time primitive. Never put the operator token in frontend configuration, responses, examples with real values or logs.

A real edit writes the stats, a normal before/after notification event and a catalog revision increment in one transaction. Use the same catalog-state lock order and shared difference/event helpers as sync, so edits serialize with sync writes. Return the updated Pokémon, catalog revision and optional change ID. Identical values are a no-op (no event/revision increase). Require an initial full successful import before demo edits. Notifications follow existing current-team ownership, deduplication and seven-day visibility rules; the frontend's Changes page already polls/refreshes them.

Manual edits have no sync run. Migration 0005 makes pokemon_changes.run_id nullable; existing events retain their run IDs and each manual edit creates its own event. This keeps sync counts, status and scheduling truthful. Change run pruning from NOT IN to NOT EXISTS so manual NULL sources cannot prevent old run cleanup. Shared helpers preserve existing per-Pokémon/per-run merging for actual sync events. A later normal sync restores authoritative upstream stats and creates the corresponding change alert; there is no persistent override layer.

Native/Compose configuration will expose the optional server variable, including preservation in AWS's private runtime environment. Default deployments remain disabled. This task adds code, contract/docs and isolated tests; it does not enable or deploy a public demo editor, migrate live data or make a sample edit to another user's teams.

## Verification

Test default-disabled and wrong/missing-token behavior, cookie-only denial, input validation, unknown IDs and initial-import gating, transactional rollback, revision/ETag changes, no-op behavior, shared-team notifications/owner isolation, unchanged sync scheduling/status, NULL-source pruning and upstream restoration. Regenerate OpenAPI/TypeScript definitions and run backend/contract/frontend checks.

## Results

Implemented the protected PATCH endpoint, partial stat updates and atomic notification/revision writes. Moved existing difference detection and event recording into shared notification helpers. Manual events use a NULL source run; ordinary sync events keep their run IDs and per-run merge behavior. Updated source-run pruning, config/Compose/private-runtime wiring, OpenAPI/TypeScript contracts and an operator curl walkthrough. The demo flag remains unset by default.

All 34 backend tests pass (11 unit, 23 integration), including real request-to-notification behavior, current-team visibility across owners, no-op edits, independent sync status/scheduling, upstream restoration, NULL-source pruning, weak/malformed token configuration rejection, Origin/content-type/auth validation and transaction rollback when history insertion fails. All 20 frontend component/API tests and 17 offline AWS tests pass. Clippy, formatting, OpenAPI/type drift, frontend production build, shell syntax, both Compose configurations and git whitespace checks passed. No browser UI was changed.

No migration, demo stat edit, service restart or deployment was performed against local development data or AWS. These changes remain uncommitted. The hosted sync schedule was checked read-only during this task: last successful full import was 2026-11-16 14:52:25 UTC, so the configured daily worker is next due around 2026-11-17 14:52:25 UTC (09:52 Eastern). Manual demo edits do not alter that deadline.
