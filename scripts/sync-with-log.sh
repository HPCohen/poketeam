#!/usr/bin/env bash
set -euo pipefail
umask 077
repo_root=$(cd "$(dirname "$0")/.." && pwd)
log_dir=${SYNC_LOG_DIR:-"$repo_root/logs"}
mkdir -p "$log_dir"
log_file="$log_dir/sync-$(date -u +%Y%m%dT%H%M%SZ)-$$.log"
(set -o noclobber; : > "$log_file")
printf 'Sync log: %s\n' "$log_file"
export RUST_LOG="${RUST_LOG:-poketeam=info}"
cargo run --locked --manifest-path "$repo_root/backend/Cargo.toml" -- sync 2>&1 | tee "$log_file"
