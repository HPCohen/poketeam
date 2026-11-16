#!/usr/bin/env bash
set -euo pipefail
source "$(dirname "$0")/common.sh"
stack_outputs
python3 "$AWS_DIR/manage.py" print-outputs "$STATE_DIR/stack.json"
aws ec2 describe-instance-status --instance-ids "$(output InstanceId)" --include-all-instances --output table
aws ssm describe-instance-information --filters "Key=InstanceIds,Values=$(output InstanceId)" --query 'InstanceInformationList[].{Instance:InstanceId,Status:PingStatus}' --output table
printf 'Session: aws ssm start-session --target %s --region %s\n' "$(output InstanceId)" "$AWS_REGION"
