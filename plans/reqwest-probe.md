# Minimal reqwest diagnostic binary

Add a standalone `reqwest-probe` binary under `backend/src/bin/` using the existing pinned reqwest, Tokio, anyhow, and serde_json dependencies. It accepts one or more JSON URLs, defaults to the previously failing Pokémon 4 endpoint, fetches them sequentially using one client, decodes with `Response::json`, retains the decoded values in a vector, prints each complete JSON response, and exits. It does not initialize application configuration or connect to PostgreSQL.

Use the same 30-second total timeout, 10-second connection timeout, TLS features, and User-Agent as sync to keep the comparison useful. Leave reqwest's ordinary redirect handling in place and omit the sync layer's URL policy, models, normalization, sprites, and database work. Log URL, response status/protocol/length, header timing, complete-body timing, and full errors. Accepting repeated URLs lets a run exercise connection reuse without adding flags or a diagnostic framework.

Set the Cargo package's default run target to `poketeam` so adding another binary preserves all existing `cargo run ... -- sync/serve/openapi` commands. Record usage in operations documentation. This is a reversible diagnostic addition without a data/schema or hosting decision.

Validation: build, formatting and Clippy; run against a small local HTTP fixture to check full JSON output, sequential requests, and nonzero failures; then capture a real PokéAPI run in a separate ignored log file. Record observations below without treating a client timeout as proof of an upstream outage.

## Verification results

- Offline build, Clippy with warnings denied, formatting, OpenAPI drift (including the preserved default run target), and whitespace checks pass.
- A temporary local HTTP/1.1 fixture verified complete JSON output for two sequential requests, retained response count, successful execution with a deliberately invalid DATABASE_URL, and nonzero exits with URL/context for malformed JSON and HTTP 404. The probe never consulted database configuration.
- Live run requested Pokémon 4 twice, then Pokémon 1, through one shared client. All three responses completed successfully over HTTP/1.1. Header/body totals: 0.464s/5.752s, 0.253s/5.559s, and 0.286s/4.285s. The run retained three decoded responses and exited 0.
- Full decoded JSON and timings are saved in `logs/reqwest-probe-20261115.log` (approximately 1.4 MiB, ignored by Git).
- The earlier sync timeout did not reproduce in this small run. This demonstrates that reqwest can fetch these endpoints in this environment; it does not establish whether earlier failures were intermittent or related to the sync request sequence/client configuration. No timeout, TLS feature, or application sync behavior was changed by adding the probe.
