#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
generated=$(mktemp)
trap 'rm -f "$generated"' EXIT
cargo run --quiet --locked --manifest-path backend/Cargo.toml -- openapi > "$generated"
diff -u backend/openapi.json "$generated"
