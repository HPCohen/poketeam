#!/usr/bin/env bash
# Uses a local HTTP fixture and an isolated SQLx test database, never live PokéAPI.
set -euo pipefail
cd "$(dirname "$0")/.."
export DATABASE_URL="${DATABASE_URL:-postgres://poketeam:poketeam@localhost:5432/poketeam}"
cargo test --locked --manifest-path backend/Cargo.toml --test integration change_notification_demo -- --nocapture
