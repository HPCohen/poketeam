# Low-cost AWS hosting scripts

Requested scope: prepare AWS CLI scripts that create hosting for the existing backend and future frontend. No resources are created during implementation. All deployment choices remain reviewable before running the scripts.

## Decisions and rationale

Use one ARM EC2 instance with the existing API, worker, PostgreSQL, and a small nginx reverse proxy. Future React build output is served by the same proxy and origin. This avoids a second database instance, ALB, NAT gateway, container cluster, paid image registry, and a frontend server process. The tradeoff is one failure domain and short deployment downtime; this is intended for a low-traffic personal/demo application, not high availability. The database remains PostgreSQL and can later move to RDS using a normal dump/restore.

CloudFront supplies a free HTTPS hostname, redirects viewers to HTTPS, and forwards API cookies and Origin headers without caching personalized responses. A Free flat-rate subscription and its WAF ACL are explicitly managed; creating a pay-as-you-go distribution alone is not equivalent. Origin HTTP is restricted to CloudFront's managed address ranges and a random origin header, but the CloudFront-to-instance hop is not application-layer TLS. The Free plan does not support VPC origins. A custom origin domain/certificate or a different tier is a separate future choice for end-to-end TLS. Nothing listens publicly on PostgreSQL or the raw API port, and administration uses SSM with no SSH ingress.

The stack defaults to us-east-1, where the global WAF/CloudFront subscription resources can be created alongside regional resources. Use t4g.small during AWS's currently documented trial through 2026-12-31; use t4g.micro for new deployments afterward. Selection is configurable and not a promise of coverage. Standard CPU credits prevent Unlimited surplus-credit charges. Build the ARM image locally, not on the small instance. Small encrypted gp3 root/data volumes and bounded Docker logs limit disk costs; data and a private S3 deployment/backup bucket survive stack deletion. Daily database dumps have finite S3 retention. Retaining these resources deliberately continues small storage charges until explicitly removed.

Free Tier eligibility must be checked against the real account, shared usage, credits, and expiry. New accounts use the post-July-2025 credit program; legacy 12-month benefits cannot be assumed. Scripts report the account plan and refuse unknown/expired/no-credit coverage unless the operator explicitly allows paid resources. They never upgrade the AWS account plan or subscribe to a paid CloudFront tier. A budget alert is advisory, not a spending cap. Public IPv4, EBS, S3 requests/storage, and traffic outside allowances can still cost money.

Implementation and verification results will be recorded below. No architectural change to the backend is required.

## Delivered files and behavior

- `aws/README.md` and `config.example.sh`: exact workflow, prerequisites, current sources/expiry, single-host and origin-HTTP tradeoffs, costs, maintenance, backups, restoration, and full cleanup. Region is explicitly limited to us-east-1 for this starter.
- `preflight.sh`: read-only account/credit checks, CLI capability check, available ARM instance AZ intersection, official AL2023 AMI, and CloudFront managed prefix list. No account-plan upgrades or paid subscriptions are requested.
- `create.sh`, `stack.template.json`, `template.py`: reviewable CloudFormation resources, private retained bucket, encrypted retained data volume, IMDSv2, Standard credits, SSM role, closed SSH/database/API ports, CloudFront/WAF Free subscription, traffic disabled initially, and account-wide budget notification. Existing healthy infrastructure is not automatically replaced.
- `deploy-backend.sh` / `host/`: local ARM build, private S3 transfer, root-only SSM activation, persistent on-host database credentials, pre-migration dumps, bounded container memory/logs, proxy/API liveness and origin-header checks, and daily systemd dumps. The data mount is required before Docker startup. Backend release images keep current/previous versions; rollout failures require explicit inspection/recovery.
- `deploy-frontend.sh`: accepts an existing static build, rejects symlinks/special files, uploads privately, and atomically switches same-origin static content without introducing frontend services or implementing a frontend.
- `backup-now.sh`, `status.sh`, `destroy.sh`, `purge-data.sh`: operations, guarded stack deletion with retained resource inventory, and separately acknowledged permanent cleanup that checks account identity and refuses an attached data volume.
- `common.sh` / `manage.py`: shared CLI polling, live FREE subscription validation, and standard-library JSON/shell-quoting helpers. Generated state is private, scoped by account/region/stack, and excluded from both Git and Docker context. The origin token never appears literally in an SSM command.
- `aws/tests/` / `requirements-dev.txt` / CI: fake CLI lifecycle tests, resource/cost/data invariants, pinned CloudFormation linting, shell syntax checks. Root README and operations docs link the new AWS workflow. Existing backend application code is unchanged.

## Verification

- **10 offline tests pass**: unknown/expired/no-credit and unavailable-instance guards, trial expiry, AZ selection, repeatable creation/token preservation/private parameter files, ARM build intent, previous-parameter preservation, rejecting paid CloudFront tiers, static symlink rejection, frontend activation command, and guarded teardown/permanent cleanup.
- Rendered CloudFormation passes **cfn-lint 1.60.1** for us-east-1, with no errors or warnings. Template size is under CloudFormation's inline-body limit; user data remains under EC2's limit.
- Production Docker Compose parses successfully. Its resolved configuration exposes no PostgreSQL host port, binds the raw API to 127.0.0.1, and exposes only the nginx origin port.
- All AWS/host shell scripts pass Bash syntax checks; CI YAML parses; `git diff --check` passes. AWS checks are added as a separate offline CI job.
- Validation tools were installed only in a temporary virtual environment. No actual AWS credentials/account/resources were accessed, and no provisioning or real deployment was attempted. AWS CLI is not installed in this workspace, so live account permissions, plan eligibility, service availability, resource creation, and host startup still require running the documented workflow in the user's configured account. The tests deliberately do not imply that AWS has accepted or run this stack.

No unresolved implementation questions block review. Review the account's real credits/shared usage and the origin-HTTP/single-host tradeoffs before running `create.sh`; those scripts are the concrete proposed deployment, not an already applied infrastructure decision.
