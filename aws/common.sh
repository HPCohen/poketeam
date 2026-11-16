#!/usr/bin/env bash
set -euo pipefail
AWS_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
REPO_DIR=$(cd "$AWS_DIR/.." && pwd)
if [[ -f "$AWS_DIR/config.sh" ]]; then source "$AWS_DIR/config.sh"; fi
export AWS_REGION=${AWS_REGION:-us-east-1}
export AWS_DEFAULT_REGION=$AWS_REGION
export AWS_PAGER=""
export AWS_CLI_AUTO_PROMPT=off
export STACK_NAME=${STACK_NAME:-poketeam}
[[ $AWS_REGION == us-east-1 || $AWS_REGION == us-east-2 ]] || { echo 'Supported regions: us-east-1 and us-east-2.' >&2; exit 1; }
[[ $STACK_NAME =~ ^[a-z][a-z0-9-]{0,30}$ ]] || { echo 'STACK_NAME must be lowercase, start with a letter, and be at most 31 characters.' >&2; exit 1; }
command -v aws >/dev/null || { echo 'Install current AWS CLI v2 first; see aws/README.md.' >&2; exit 1; }
command -v python3 >/dev/null || { echo 'python3 is required.' >&2; exit 1; }
umask 077
ACCOUNT_ID=$(aws sts get-caller-identity --query Account --output text)
[[ $ACCOUNT_ID =~ ^[0-9]{12}$ ]] || { echo 'Unable to resolve AWS account ID.' >&2; exit 1; }
STATE_DIR="$AWS_DIR/.state/$ACCOUNT_ID/$AWS_REGION/$STACK_NAME"
mkdir -p "$STATE_DIR"
aws_json() { aws "$@" --output json; }
stack_outputs() {
    aws_json cloudformation describe-stacks --stack-name "$STACK_NAME" > "$STATE_DIR/stack.json"
    if [[ $AWS_REGION == us-east-2 && -f "$STATE_DIR/edge.json" ]] &&
        python3 -c 'import json,sys;sys.exit("distribution_id" not in json.load(open(sys.argv[1])))' "$STATE_DIR/edge.json"; then
        python3 "$AWS_DIR/edge.py" outputs "$STATE_DIR"
    fi
}
output() {
    python3 - "$STATE_DIR/stack.json" "$1" <<'PY'
import json,sys
outputs=json.load(open(sys.argv[1]))['Stacks'][0].get('Outputs',[])
print(next(o['OutputValue'] for o in outputs if o['OutputKey']==sys.argv[2]))
PY
}
wait_ssm() {
    local instance=$1 count status
    for count in {1..90}; do
        status=$(aws ssm describe-instance-information --filters "Key=InstanceIds,Values=$instance" --query 'InstanceInformationList[0].PingStatus' --output text)
        [[ $status == Online ]] && return
        sleep 10
    done
    echo 'SSM agent did not become ready; inspect EC2 console system logs/cloud-init.' >&2
    return 1
}
remote() {
    local instance=$1 script=$2 command_id count status
    python3 - "$script" "$STATE_DIR/command.json" <<'PY'
import json,sys
json.dump({'commands':[open(sys.argv[1]).read()], 'executionTimeout':['3600']},open(sys.argv[2],'w'))
PY
    command_id=$(aws ssm send-command --instance-ids "$instance" --document-name AWS-RunShellScript --parameters "file://$STATE_DIR/command.json" --timeout-seconds 600 --query Command.CommandId --output text)
    for count in {1..360}; do
        # Invocation records appear asynchronously; distinguish eventual absence from other failures.
        if aws_json ssm get-command-invocation --command-id "$command_id" --instance-id "$instance" > "$STATE_DIR/invocation.json" 2> "$STATE_DIR/invocation.err"; then
            status=$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["Status"])' "$STATE_DIR/invocation.json")
            case "$status" in
                Success) python3 "$AWS_DIR/manage.py" print-invocation "$STATE_DIR/invocation.json"; return ;;
                Failed|Cancelled|TimedOut|Cancelling) python3 "$AWS_DIR/manage.py" print-invocation "$STATE_DIR/invocation.json"; echo "SSM command $command_id ended: $status" >&2; return 1 ;;
            esac
        elif ! grep -q InvocationDoesNotExist "$STATE_DIR/invocation.err"; then
            cat "$STATE_DIR/invocation.err" >&2; return 1
        fi
        sleep 10
    done
    echo "SSM command $command_id is still running; inspect it in Systems Manager." >&2
    return 1
}

require_edge_configuration() {
    if [[ $AWS_REGION == us-east-2 ]]; then
        python3 "$AWS_DIR/edge.py" check "$STATE_DIR"
        return
    fi
    aws_json pricing-plan-manager get-subscription --arn "$(output SubscriptionArn)" > "$STATE_DIR/subscription.json"
    python3 - "$STATE_DIR/subscription.json" <<'PYCODE'
import json,sys
subscription=json.load(open(sys.argv[1]))['subscription']
if subscription['status'] != 'ACTIVE' or subscription['planTier'] != 'FREE' or subscription.get('scheduledChange'):
    sys.exit('CloudFront must have an ACTIVE FREE subscription without pending cancellation/tier changes.')
PYCODE
}
