#!/usr/bin/env bash
# Permanently removes retained database/backups after destroy.sh. No recovery afterward.
set -euo pipefail
source "$(dirname "$0")/common.sh"
account=$(aws sts get-caller-identity --query Account --output text)
[[ ${CONFIRM_PURGE:-} == "$STACK_NAME:$account" ]] || { echo "Permanent data deletion requires CONFIRM_PURGE=$STACK_NAME:$account" >&2; exit 1; }
[[ -f "$STATE_DIR/retained-logical-resources.json" ]] || { echo 'Run destroy.sh first; retained resource IDs are required.' >&2; exit 1; }
python3 "$AWS_DIR/manage.py" retained-values "$STATE_DIR" "$account"
volume=$(python3 -c 'import json,sys;print(json.load(open(sys.argv[1]))["volume"])' "$STATE_DIR/retained-data.json")
bucket=$(python3 -c 'import json,sys;print(json.load(open(sys.argv[1]))["bucket"])' "$STATE_DIR/retained-data.json")
# A still-attached volume must never be removed, even with the purge acknowledgement.
if [[ -n $volume ]]; then
    state=$(aws ec2 describe-volumes --volume-ids "$volume" --query 'Volumes[0].State' --output text)
    [[ $state == available ]] || { echo "Volume $volume is $state; refusing purge." >&2; exit 1; }
fi
if [[ -n $bucket ]]; then
    aws s3api head-bucket --bucket "$bucket" --expected-bucket-owner "$account"
    aws s3 rm "s3://$bucket" --recursive --only-show-errors
    aws s3api delete-bucket --bucket "$bucket" --expected-bucket-owner "$account"
fi
[[ -z $volume ]] || aws ec2 delete-volume --volume-id "$volume"
aws ssm delete-parameter --name "/$STACK_NAME/origin-token"
echo 'Retained database disk, S3 bucket/backups, and origin token permanently deleted.'
