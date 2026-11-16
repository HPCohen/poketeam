#!/usr/bin/env bash
# Delete the standard-pricing global edge with explicit confirmation.
set -euo pipefail
source "$(dirname "$0")/common.sh"
[[ ${1:-} == destroy && ${CONFIRM_DESTROY:-} == "$STACK_NAME" ]] || { echo "Usage: CONFIRM_DESTROY=$STACK_NAME aws/edge.sh destroy" >&2; exit 1; }
python3 "$AWS_DIR/edge.py" destroy "$STATE_DIR"
