# Capture sync failures with URLs

The user requested a separate log file identifying the failing URL and error after an upstream body timeout lacked URL context.

Add structured upstream request failure logs with the current URL (including redirect destination), one-based attempt number, and complete error chain. Add final request and JSON decoding URL context so captured CLI stderr remains useful even when tracing is filtered. Keep retry classification, limits, timeouts, and atomic publication unchanged. The database/catalog status still stores only the existing short summary.

Add `scripts/sync-with-log.sh` to capture combined stdout/stderr with `tee` in a exclusive UTC-stamped, process-ID-suffixed `logs/sync-*.log`. It builds the current code through Cargo, prints the log path, and preserves sync failure exit status through `pipefail`. Logs are ignored by Git. `SYNC_LOG_DIR` optionally selects another directory; normal backend environment variables still apply. The user explicitly requested URL capture; no new irreversible architecture or data decision is introduced.

Validation: exercise CLI sync failure against the existing local mock fixture, checking URL, underlying error, attempt numbers, exit status, and stored summary. Run formatting and backend checks, then run a live sync through the logging script and record the file/outcome below.

Verification: all 20 backend tests (9 unit, 11 integration), Clippy with warnings denied, formatting, and shell syntax checks pass. The regression test confirms three failed attempts carry the URL/error and the CLI exits unsuccessfully while the database keeps its short summary. A live sync is writing combined output to the separate log file.

Live capture completed: `logs/sync-20261115T202300Z.log`. The command exited 1 after the third failed attempt to fetch `https://pokeapi.co/api/v2/pokemon/1/`; the complete chain ends with `operation timed out` while decoding the response body. Other failed requests and their attempts are also recorded. The wrapper preserved the failing status, and no partial catalog was published.

The later [sync simplification](sync-simplification.md) replaces per-request retries with first-failure behavior and reqwest JSON decoding. URL/error capture remains; request attempt counters are no longer emitted.
