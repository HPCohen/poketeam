#!/usr/bin/env bash
# Upload already-built React/Vite/etc output; no frontend implementation is assumed.
set -euo pipefail
source "$(dirname "$0")/common.sh"
[[ $# == 1 && -f "$1/index.html" ]] || { echo 'Usage: aws/deploy-frontend.sh path/to/dist (must contain index.html)' >&2; exit 1; }
stack_outputs
bucket=$(output BucketName)
instance=$(output InstanceId)
release="frontend-$(date -u +%Y%m%dT%H%M%SZ)-$(python3 -c 'import secrets;print(secrets.token_hex(4))')"
# Reject symlinks and special files before packaging static output.
python3 "$AWS_DIR/manage.py" validate-frontend "$1"
tar -czf "$STATE_DIR/frontend.tar.gz" -C "$1" .
aws s3 cp "$STATE_DIR/frontend.tar.gz" "s3://$bucket/releases/$release.tar.gz" --sse AES256 --only-show-errors
python3 "$AWS_DIR/manage.py" frontend-command "$STATE_DIR/frontend.sh" "$bucket" "$release"
wait_ssm "$instance"
remote "$instance" "$STATE_DIR/frontend.sh"
# Cache is disabled at the distribution, so no billed invalidation is required.
printf 'Frontend: %s\n' "$(output PublicOrigin)"
