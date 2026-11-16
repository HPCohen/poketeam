# Copy to aws/config.sh (gitignored). AWS CLI uses your normal credentials/profile.
export AWS_PROFILE=default
export AWS_REGION=us-east-1
export STACK_NAME=poketeam
export BUDGET_EMAIL=you@example.com
export MONTHLY_BUDGET_USD=5
# Empty selects t4g.small until 2026-12-31, then t4g.micro for new stacks.
export INSTANCE_TYPE=
# Set only after reviewing real account coverage/costs if eligibility cannot be verified.
export ALLOW_PAID_RESOURCES=0
# 16 GiB root + 8 GiB data; both encrypted gp3. Data survives stack deletion.
export ROOT_GIB=16
export DATA_GIB=8
