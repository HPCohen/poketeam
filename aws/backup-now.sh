#!/usr/bin/env bash
set -euo pipefail
source "$(dirname "$0")/common.sh"
stack_outputs
printf '#!/bin/bash\nset -euo pipefail\nexec bash /srv/poketeam/current/backup.sh\n' > "$STATE_DIR/backup-now.sh"
wait_ssm "$(output InstanceId)"
remote "$(output InstanceId)" "$STATE_DIR/backup-now.sh"
