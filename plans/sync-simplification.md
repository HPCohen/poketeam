# Simplify catalog synchronization

## Requested change

Use a simple paginated loop over the upstream API, collect results in vectors, and then write the corresponding database tables. Decode JSON with reqwest's built-in decoder into Serde types in the model module.

## Implementation

- `model/pokeapi.rs` holds the upstream page/resource, species/default variety, Pokémon/stat/type/sprite, and damage-relation response models. A `TryFrom` conversion handles the upstream Pokémon shape versus our existing normalized catalog row. The public API and database schema stay compatible.
- Sync uses sequential `while`/`for` loops: follow each page's `next` URL into a vector, fetch species/default Pokémon and type details, normalize them, and collect rows before writing. The upstream lists contain resource links rather than complete Pokémon records, so detail requests remain necessary.
- JSON requests use reqwest `send`, `error_for_status`, and `Response::json::<T>()`. Binary sprites use reqwest `Response::bytes()`. Remove the custom streaming reader, semaphore, concurrent futures pipelines, retry/backoff/Retry-After loop, and manual redirect walker. Remove the direct futures dependency and switch reqwest's `stream` feature to `json`.
- Reqwest handles redirects through a short policy preserving the configured host/scheme restrictions and five-hop limit. Keep request/connect timeouts and advertised/downloaded sprite size checks. JSON bodies use reqwest's standard decoder rather than a custom size-limited streaming buffer.
- Requests stop the pass on the first HTTP, network, or decoding error. The worker still retries the whole failed pass after 15 minutes. URL and full error context still reach the separate sync log; the obsolete per-request attempt counter is removed.
- Keep complete-list checks before treating missing Pokémon as unavailable. Retain the existing transaction for Pokémon, sprite assets, type effectiveness, change events, revision, and run status. This is required by current team retention and notification behavior and prevents a partially fetched list from invalidating existing members. No migrations, data deletion, API contract change, or irreversible architecture decision is introduced.

## Validation

Update integration checks for first-failure behavior, paginated species and type lists, unchanged failed-pass publication, size checks, scheduling and restart recovery, and URL/error logging. Add malformed upstream JSON coverage for reqwest decoding and preserved catalog data. Run the backend tests, formatting, Clippy, and OpenAPI drift check. Run one logged live sync after checks, using the existing development database; report any upstream failures without claiming a successful import.

## Check results

- All 21 backend tests pass: 9 unit tests and 12 isolated PostgreSQL/mock-HTTP integration tests.
- Species and type pagination both reach their second page and populate eight Pokémon and 64 effectiveness rows. A failed HTTP request is attempted once, preserves the previous revision, and retains URL/error logging.
- Malformed JSON is rejected by reqwest's decoder, logged with its URL and decode error, and leaves the existing catalog unchanged.
- Allowed relative redirects work and disallowed-host redirects fail. Existing sprite changes, unavailable-member retention, notifications, locking, cancellation, and worker scheduling checks pass.
- Clippy with warnings denied, Rust formatting, OpenAPI drift, and whitespace checks pass.
- Live sync used the refactored reqwest JSON path and stopped on a response-body timeout at `https://pokeapi.co/api/v2/pokemon/4/`. Full URL/error context is saved in `logs/sync-20261115T204403Z-42280.log`; exit status was 1 and no partial catalog was published. This verifies the live request/error path, but a complete live catalog import remains unverified because of the upstream timeout.

The later [incremental sync change](incremental-sync.md), explicitly requested by the user, supersedes whole-snapshot collection/publication with per-Pokémon commits and partial-progress visibility. The reqwest decoder, models, sequential fetches, and first-failure behavior remain.
