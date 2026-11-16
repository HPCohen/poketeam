#!/usr/bin/env bash
# Creates resources only when explicitly run by the operator.
set -euo pipefail
source "$(dirname "$0")/common.sh"
"$AWS_DIR/preflight.sh"
[[ ${BUDGET_EMAIL:-} == *@* && ${BUDGET_EMAIL:-} != you@example.com ]] || { echo 'Set BUDGET_EMAIL in aws/config.sh first.' >&2; exit 1; }
if aws_json cloudformation describe-stacks --stack-name "$STACK_NAME" > "$STATE_DIR/stack.json" 2> "$STATE_DIR/stack.err"; then
    python3 "$AWS_DIR/manage.py" check-stack "$STATE_DIR/stack.json"
    echo 'Stack already exists; infrastructure is not replaced automatically. Use deploy-backend.sh for application updates.'
    if [[ $AWS_REGION == us-east-2 ]]; then
        aws_json ssm get-parameter --name "/$STACK_NAME/origin-token" --with-decryption > "$STATE_DIR/token.json"
        python3 "$AWS_DIR/edge.py" create "$STATE_DIR"
    fi
    python3 "$AWS_DIR/manage.py" print-outputs "$STATE_DIR/stack.json"
    exit 0
elif ! grep -q 'does not exist' "$STATE_DIR/stack.err"; then
    cat "$STATE_DIR/stack.err" >&2; exit 1
fi
# The secure parameter survives stack deletion. Never rotate it implicitly on reruns.
if ! aws_json ssm get-parameter --name "/$STACK_NAME/origin-token" --with-decryption > "$STATE_DIR/token.json" 2> "$STATE_DIR/token.err"; then
    if ! grep -q ParameterNotFound "$STATE_DIR/token.err"; then cat "$STATE_DIR/token.err" >&2; exit 1; fi
    python3 "$AWS_DIR/manage.py" new-token "$STATE_DIR/token-request.json"
    aws ssm put-parameter --cli-input-json "file://$STATE_DIR/token-request.json" >/dev/null
    aws_json ssm get-parameter --name "/$STACK_NAME/origin-token" --with-decryption > "$STATE_DIR/token.json"
fi
python3 "$AWS_DIR/template.py" > "$STATE_DIR/template.json"
python3 "$AWS_DIR/manage.py" parameters "$STATE_DIR" false
aws cloudformation validate-template --template-body "file://$STATE_DIR/template.json" >/dev/null
aws cloudformation deploy --stack-name "$STACK_NAME" --template-file "$STATE_DIR/template.json" --parameter-overrides "file://$STATE_DIR/parameters.json" --capabilities CAPABILITY_IAM --tags "Project=$STACK_NAME" --no-fail-on-empty-changeset
stack_outputs
if [[ $AWS_REGION == us-east-2 ]]; then python3 "$AWS_DIR/edge.py" create "$STATE_DIR"; fi
require_edge_configuration
# Keep traffic disabled until backend deployment has installed the proxy/configuration.
echo 'Infrastructure created with public traffic disabled. Next: aws/deploy-backend.sh'
python3 "$AWS_DIR/manage.py" print-outputs "$STATE_DIR/stack.json"
