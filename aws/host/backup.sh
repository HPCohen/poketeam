#!/usr/bin/env bash
set -euo pipefail
mountpoint -q /srv/poketeam
[[ -f /srv/poketeam/current/compose.yaml ]]
source /srv/poketeam/runtime.env
backup_key="backups/$(date -u +%Y-%m-%dT%H-%M-%SZ).dump"
docker compose --env-file /srv/poketeam/runtime.env -f /srv/poketeam/current/compose.yaml exec -T db pg_dump -U poketeam -d poketeam -Fc |
    aws s3 cp - "s3://$BUCKET/$backup_key" --sse AES256 --only-show-errors
printf 'Database backup: s3://%s/%s\n' "$BUCKET" "$backup_key"
