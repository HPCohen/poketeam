#!/usr/bin/env bash
# Deletes hosting resources, retaining database disk, bucket, and origin-token parameter.
set -euo pipefail
source "$(dirname "$0")/common.sh"
[[ ${CONFIRM_DESTROY:-} == "$STACK_NAME" ]] || { echo "Run with CONFIRM_DESTROY=$STACK_NAME to remove hosting (retains paid storage)." >&2; exit 1; }
stack_outputs
if [[ $AWS_REGION == us-east-2 ]]; then python3 "$AWS_DIR/edge.py" destroy "$STATE_DIR"; fi
cp "$STATE_DIR/stack.json" "$STATE_DIR/retained-resources.json"
aws_json cloudformation list-stack-resources --stack-name "$STACK_NAME" > "$STATE_DIR/retained-logical-resources.json"
aws cloudformation delete-stack --stack-name "$STACK_NAME"
aws cloudformation wait stack-delete-complete --stack-name "$STACK_NAME"
echo 'Hosting deleted. Retained EBS volume and S3 data still incur storage charges:'
python3 "$AWS_DIR/manage.py" print-outputs "$STATE_DIR/retained-resources.json"
echo "Origin token is retained at /$STACK_NAME/origin-token. See README for permanent data removal."
