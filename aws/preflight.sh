#!/usr/bin/env bash
set -euo pipefail
source "$(dirname "$0")/common.sh"
if [[ $AWS_REGION == us-east-1 ]]; then
    aws pricing-plan-manager get-subscription --generate-cli-skeleton input >/dev/null
fi
aws_json sts get-caller-identity > "$STATE_DIR/identity.json"
if ! aws_json freetier get-account-plan-state > "$STATE_DIR/plan.json" 2> "$STATE_DIR/plan.err"; then
    printf '{}\n' > "$STATE_DIR/plan.json"
    cat "$STATE_DIR/plan.err" >&2
fi
python3 "$AWS_DIR/manage.py" preflight "$STATE_DIR/identity.json" "$STATE_DIR/plan.json"
# These are account/region queries, not resource creation.
aws_json ec2 describe-instance-type-offerings --location-type availability-zone --filters "Name=instance-type,Values=$(cat "$STATE_DIR/instance-type")" > "$STATE_DIR/offerings.json"
python3 "$AWS_DIR/manage.py" check-offerings "$STATE_DIR/offerings.json"
aws_json ec2 describe-availability-zones --filters Name=state,Values=available > "$STATE_DIR/zones.json"
aws ssm get-parameter --name /aws/service/ami-amazon-linux-latest/al2023-ami-kernel-default-arm64 --query Parameter.Value --output text > "$STATE_DIR/ami-id"
aws ec2 describe-managed-prefix-lists --filters Name=prefix-list-name,Values=com.amazonaws.global.cloudfront.origin-facing --query 'PrefixLists[0].PrefixListId' --output text > "$STATE_DIR/prefix-list-id"

if [[ $AWS_REGION == us-east-2 ]]; then
    # These service APIs are global; CloudFormation in us-east-1 is not required.
    aws_json cloudfront list-distributions --region us-east-1 > "$STATE_DIR/distributions.json"
fi
