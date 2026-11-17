# CI workflow fixes

The November 17, 2026 runs for commit `1e03b65` exposed two failures:

- Frontend run `44021692584`: `ubuntu-latest` selected Ubuntu 26.04, which the pinned Playwright 1.58.0 cannot install Chromium on.
- Backend run `44021692592`: the executable lifecycle test exceeded its two-second SIGTERM shutdown deadline. The exact failing subprocess was not identified in the CI output; the same test passed locally.

## Changes

Keep the frontend job on `ubuntu-latest` and upgrade the exact Playwright dependency from 1.58.0 to 1.65.1, including its lockfile. Current Playwright supports Ubuntu 26.04, so no runner pin or container is needed. The supported platforms are documented at https://playwright.dev/docs/intro#system-requirements. The registry reports 1.65.1 as the current stable release. Keeping an exact version preserves reproducible installs; future Ubuntu migrations may still require a Playwright update.

Keep one shutdown listener active around the entire worker loop. Previously each scheduling, scan, and retry phase created a new listener, and the database scheduling query had no shutdown branch. Signals could be lost between listeners or shutdown could wait on the database. The worker now cancels scheduling, scans, and retry waits through one outer `select!`, preserving existing scheduling and transaction behavior.

Use a dedicated PostgreSQL connection for the scheduling query, as the synchronization operation already does. Cancelling a query on a pooled connection can leave SQLx draining that query while `pool.close()` waits. Dropping a dedicated connection closes the socket immediately. This adds one short-lived connection per scheduling check, normally once per daily scan. The regression test reproduced the timeout with only the outer shutdown listener change and passed after this connection fix.

Add a Unix integration regression test that holds an exclusive lock on `sync_runs`, observes the worker blocked in its scheduling query, and verifies SIGTERM exits successfully before releasing the lock. Keep the existing shutdown deadline instead of hiding the failure with a longer timeout.

The upgraded browser exposed a mobile E2E timing race: the keyboard test tried to focus Save immediately after typing, while React could still have the button disabled. Wait for it to become enabled and assert focus before pressing Enter, preserving the keyboard interaction being tested.

## Validation

Backend: all 11 unit tests and 24 integration tests passed, including the existing executable lifecycle test and the new blocked scheduling regression.

Frontend: typechecking, 20 unit tests, production build, and generated API type comparison passed. The API comparison ran the existing renamed `scripts/check-api.test` as an ES module because the working tree already has a user change removing `check-api.mjs`; those script changes are outside this fix.

Formatting, Clippy with warnings denied, and OpenAPI verification passed. A browser installation dry run using Playwright's `ubuntu26.04-x64` platform override selected the Linux Chromium binaries without the previous unsupported-platform error.

All eight browser E2E tests passed across desktop Chromium and mobile emulation after the keyboard test fix. GitHub Actions itself has not been rerun; changes are local. Native Ubuntu dependency installation has not been executed on this macOS host.
