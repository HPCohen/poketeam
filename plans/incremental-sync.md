# Save each Pokémon as it is received

The user explicitly requested incremental database persistence after the minimal reqwest probe succeeded while whole-catalog syncs timed out.

## Behavior and reasons

- Fetch the complete type chart first and publish it in one short transaction. Fetch species discovery, then request each default Pokémon and its sprite sequentially. Commit that Pokémon, sprite asset, change event, catalog revision, and run progress before requesting the next Pokémon. Retain only seen IDs, not all downloaded records/assets, in memory.
- An HTTP/JSON/sprite/database failure or cancellation retains previous committed Pokémon. The current Pokémon transaction rolls back. No database transaction spans network requests. A failed run keeps its processed/count progress and URL/error log.
- Serve the partial catalog once a type chart and at least one Pokémon have committed. Increment the catalog revision on each visible change so existing ETags invalidate correctly. `last_success_at` continues to mean a completely successful scan, not partial progress.
- Initial import stays silent until the first fully successful scan, including retries after partial initial imports. Later committed changes generate notifications even if the scan eventually fails. Merge type-chart and field changes for the same Pokémon/run into one event using the existing table; no schema migration is required.
- Mark missing/default-replaced Pokémon unavailable only after complete discovery and successful processing of every resource. Finalize those availability updates, successful run metadata, and pruning together in a short transaction. Incomplete scans must never retire unvisited members.
- Keep the advisory lock for the whole scan to prevent overlapping syncs; it does not hold a publication transaction open. The worker retains its whole-pass retry schedule.

This explicitly replaces the previous whole-snapshot publication guarantee, as authorized by the user's request. Readers can observe the latest committed record updates during a scan. No data is cleared or migrated. Small per-record transactions keep existing sprite references and notifications consistent while preserving useful progress.

## Implementation and validation

Replace the all-record Snapshot upstream interface with small discovery/chart/single-Pokémon fetch methods. Reuse explicit SQL for per-record saves and successful-scan finalization. Update operations/API documentation and earlier plan notes.

Test partial first-import visibility, retained writes after a later HTTP/JSON failure, per-record atomicity, progress counts, revisions/ETags, initial notification silence, later notifications surviving failures without duplicates, no unavailable marking before scan completion, cancellation/lock recovery, and successful finalization. Run backend tests, formatting, Clippy, and OpenAPI drift checks, then run a logged live scan and report actual saved progress and limitations.

## Verification

- All 24 backend tests pass: 9 unit and 15 PostgreSQL/mock-HTTP integration tests.
- Partial initial import: two records survive a later error, are immediately browsable with readiness 200, report failed progress 2 and no completed-scan timestamp, and stay notification-silent through successful recovery.
- Failed refresh: earlier stat/sprite changes and their ETags/notifications persist; an absent but unvisited team member remains active until successful completion. Recovery does not duplicate the committed member's notification.
- A database constraint failure on a later Pokémon leaves that row unchanged while earlier commits, revision and progress persist. The first-record failure test confirms its newly inserted sprite rolls back with the row.
- Cancellation after two commits retains those rows, releases the advisory lock, and permits a new scan to finish. The process-level scheduling test now waits for full success, since readiness intentionally arrives earlier.
- Existing combined type/field notification, unavailable/default-variety retention, initial-import silence, pagination, worker scheduling, HTTP and JSON diagnostics checks pass.
- Formatting, Clippy with warnings denied, OpenAPI drift, and whitespace checks pass. The live run and resulting progress are recorded below.

## Live run

`logs/sync-20261116T021857Z-57693.log` records the per-Pokémon commits. Run `b0c7487c-59e1-45fb-9823-f169f9bad9d6` completed successfully with 1,025 processed Pokémon, zero change events, and exit status 0. Each progress entry is emitted after its transaction commits. The API reports readiness 200, successful run status and 1,025 active catalog records. The HTTP status check completed after the live pass had finished; partial visibility during an active/failing scan is covered by the isolated integration tests.

This live success does not establish why earlier network timeouts occurred. Incremental writes preserve completed work if a future request fails; they do not change reqwest's timeout configuration.

Final live smoke through Vite passed: frontend HTTP, API liveness, real catalog payload, HttpOnly session cookie/Origin validation, team creation/update/revision conflict/read/delete, and a real Power counter response. The smoke team was removed. Existing frontend/API services remain running for review.
