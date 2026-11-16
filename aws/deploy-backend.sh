#!/usr/bin/env bash
set -euo pipefail
source "$(dirname "$0")/common.sh"
command -v docker >/dev/null || { echo 'Docker with buildx is required to build linux/arm64 locally.' >&2; exit 1; }
stack_outputs
instance=$(output InstanceId)
bucket=$(output BucketName)
origin=$(output PublicOrigin)
require_free_subscription
release="$(date -u +%Y%m%dT%H%M%SZ)-$(python3 -c 'import secrets;print(secrets.token_hex(4))')"
release_dir="$STATE_DIR/releases/$release"
mkdir -p "$release_dir/host"
docker buildx build --platform linux/arm64 --load -t "poketeam:$release" "$REPO_DIR"
docker save "poketeam:$release" | gzip > "$release_dir/image.tar.gz"
cp "$AWS_DIR/host/compose.yaml" "$AWS_DIR/host/nginx.conf.template" "$AWS_DIR/host/activate.sh" "$AWS_DIR/host/backup.sh" "$release_dir/host/"
python3 - "$release_dir/release.env" "$release" "$origin" "$bucket" <<'PY'
import sys
with open(sys.argv[1],'w') as f:
 for k,v in zip(['RELEASE','PUBLIC_ORIGIN','BUCKET'],sys.argv[2:]):f.write(f'{k}={v}\n')
PY
tar -czf "$STATE_DIR/release.tar.gz" -C "$release_dir" image.tar.gz release.env host
aws s3 cp "$STATE_DIR/release.tar.gz" "s3://$bucket/releases/$release.tar.gz" --sse AES256 --only-show-errors
python3 "$AWS_DIR/manage.py" deploy-command "$STATE_DIR/deploy.sh" "$bucket" "$release" "$STACK_NAME"
wait_ssm "$instance"
remote "$instance" "$STATE_DIR/deploy.sh"
# Enable only after migration and local API liveness passed. Initial catalog import is asynchronous.
aws_json cloudformation get-template --stack-name "$STACK_NAME" --template-stage Original > "$STATE_DIR/current-template.json"
aws ec2 describe-instances --instance-ids "$instance" --query 'Reservations[0].Instances[0].PublicDnsName' --output text > "$STATE_DIR/origin-dns"
python3 "$AWS_DIR/manage.py" enable-parameters "$STATE_DIR"
aws cloudformation deploy --stack-name "$STACK_NAME" --template-file "$STATE_DIR/enabled-template.json" --parameter-overrides "file://$STATE_DIR/enable-parameters.json" --capabilities CAPABILITY_IAM --no-fail-on-empty-changeset
stack_outputs
printf 'API: %s/api/v1/catalog/status\nReadiness becomes healthy after initial catalog sync.\n' "$origin"
