# AWS hosting on a Free account

The default deployment keeps the AWS account on its Free plan. It uses an EC2 host in **us-east-2** and a **standard usage-based CloudFront distribution** for its public HTTPS URL. There is no flat-rate subscription, WAF, account upgrade, paid domain, ALB, NAT gateway, RDS instance, ECS cluster or image registry. See [the approved redesign](../plans/aws-free-account-redesign.md).

## Architecture

```text
Browser ── HTTPS ── standard CloudFront
                       │ HTTP, CloudFront prefix list + random secret header
                       ▼
                EC2 nginx :80 (us-east-2)
                 ├─ React static files / SPA fallback
                 └─ /api/*, /health/*, /openapi.json → Rust API
                       │
                   PostgreSQL + sync worker
                       │
                 encrypted retained EBS
                       └─ daily private S3 backups
```

API/PostgreSQL ports are not public. SSM provides administration without SSH ingress. API cookies, Origin and query parameters pass through CloudFront with caching disabled. The backend's configured HTTPS origin enables Secure HttpOnly cookies. The origin hop uses HTTP, restricted by a managed CloudFront prefix list and random 256-bit header; it is not end-to-end TLS. One host is a single failure domain. Nginx limits aggregate API traffic to 20 requests/second with a burst of 40; this is local protection, not a managed WAF or billing cap.

CloudFormation owns regional resources. The small `edge.py` helper manages CloudFront directly through AWS CLI because managed projects restrict CloudFormation in us-east-1. Its standard-pricing path never calls WAF or pricing-plan-manager. Stable names/caller references allow interrupted creation to resume; private IDs live under `aws/.state/<account>/<region>/<stack>/`. Preserve that directory and do not run concurrent create/deploy/destroy commands for one stack.

## Cost and account limits

A Free AWS **account** is different from CloudFront's $0 **flat-rate plan**, which Free accounts cannot use. This deployment uses standard CloudFront pricing instead. AWS documents its [standard free allowances](https://aws.amazon.com/cloudfront/faqs/) separately. Account credits and expiry still apply; this is not permanently free hosting. See [managed-project services](https://docs.aws.amazon.com/accounts/latest/reference/supported-services-sign-up-new.html) and [flat-rate account requirements](https://docs.aws.amazon.com/PricingPlanManager/latest/UserGuide/plans.html).

For 168 hours on t4g.small, 24 GB gp3, one IPv4 and light traffic, estimate roughly $4.15/week before trials/credits, or about $1.30–$1.50 if the shared t4g.small trial covers compute. The [trial](https://aws.amazon.com/ec2/faqs/) currently lasts through 2026-12-31. Review eligibility/shared usage and current pricing before deployment. Standard CPU credits avoid surplus CPU-credit fees. EC2 defaults to t4g.small during that trial, then t4g.micro for new stacks; existing hosts are never automatically resized.

`preflight.sh` checks actual identity, account plan/credits, regional instance availability, AMI, CloudFront prefix list and CloudFront read access. It refuses unknown/expired/no credits unless `ALLOW_PAID_RESOURCES=1` is explicitly set. That flag does not upgrade an account. The default $5/month budget sends an email above 80% of underlying account spend, excluding credits/refunds; it does not cap spending. Other account workloads count.

## First deployment

Install current AWS CLI v2, Python 3, Bash and Docker with buildx. Log in with the normal deployment profile. The deployment identity needs CloudFormation, EC2/VPC/EBS, IAM/PassRole, S3, SSM/KMS, CloudFront, Budgets and read access to STS/Free Tier/public AMI parameters. No credential files are copied into images or scripts.

```sh
cp aws/config.example.sh aws/config.sh
# Set AWS_PROFILE, BUDGET_EMAIL, stack name and any instance/storage overrides.
# Keep AWS_REGION=us-east-2 for the default project-compatible deployment.
aws/preflight.sh
aws/create.sh             # Regional stack + disabled standard distribution
aws/deploy-backend.sh     # ARM image, private upload, migration, API/worker/proxy, enable edge
cd frontend
npm ci
npm run build
cd ..
aws/deploy-frontend.sh frontend/dist
aws/status.sh
```

Use the printed `https://....cloudfront.net` origin. `create.sh` resumes existing healthy stacks without replacing infrastructure or rotating secrets; failed/rolled-back stacks require inspection. Initial CloudFront propagation, cloud-init and catalog sync take time. Images build locally rather than on the small host. `RUST_IMAGE` may override the Dockerfile builder image if an equivalent trusted mirror is needed.

The origin token is an SSM SecureString; CloudFront receives it through a private JSON file. The instance fetches it through its role, so the token is not embedded in SSM command history. The database password is generated on the instance and stored on its retained encrypted disk. `aws/config.sh` and `.state` are ignored by Git/build contexts; generated secret files are mode 0600. The SSM role's managed policy grants parameter reads as well as SSM administration.

Cloud-init mounts the intended data volume by serial number, refuses to format recognized data, and requires the mount before Docker startup. IMDSv2 and the metadata one-hop limit prevent normal app containers from reaching the instance role. Root/data volumes are encrypted gp3 (16/8 GB defaults).

## Verify the deployed app

```sh
curl https://YOUR_DISTRIBUTION.cloudfront.net/health/live
curl https://YOUR_DISTRIBUTION.cloudfront.net/health/ready
curl https://YOUR_DISTRIBUTION.cloudfront.net/api/v1/catalog/status
```

Readiness becomes healthy after the type chart and first Pokémon commit. Full sync completion is separate and reported by catalog status. The worker saves each Pokémon incrementally and retries failures on its persisted schedule. Open the exact public origin in the browser so same-origin mutation validation and cookies work. A CLI mutation must include the exact Origin and a JSON object.

## Updates, logs and backups

Run `aws/deploy-backend.sh` for backend updates and `aws/deploy-frontend.sh frontend/dist` after frontend builds. Existing deployments stop writes and take a private database backup before migrations. Migration/startup failures stop deployment and require inspection; there is no automatic schema downgrade. The current/previous releases/images are retained. Frontend switching uses an atomic symlink; obsolete static releases require deliberate cleanup when needed. CloudFront caching is disabled, so no invalidation is necessary.

CloudFront origin DNS is refreshed by backend deployment after an EC2 stop/start changes its public address. Its viewer hostname stays stable. Infrastructure resizing/replacement is a separate migration; deployment does not reformat or move a live database. Keep host/container security updates in the maintenance routine.

`aws/status.sh` prints the SSM session command. With the Session Manager plugin, connect and inspect:

```sh
sudo tail -n 100 /var/log/poketeam-bootstrap.log
sudo cloud-init status --long
sudo docker compose --env-file /srv/poketeam/runtime.env \
  -f /srv/poketeam/current/compose.yaml logs --tail 100 api worker
sudo systemctl status poketeam-backup.timer poketeam-backup.service
```

Container logs rotate at 3 × 10 MB/service. Daily pg_dump runs around 04:00 UTC and uploads privately to S3. `aws/backup-now.sh` triggers one immediately. Backups expire after 14 days, release objects after seven days, incomplete multipart uploads after one day. There is no paid monitoring service; verify the timer and actual backup objects. Bucket lifecycle continues after stack deletion, so download any long-term recovery copies before expiry.

Restore pg_dump into a new empty PostgreSQL database, verify teams/catalog, then switch the app URL. See [database recovery](../docs/operations.md). A retained data disk also contains runtime credentials, requires its original AZ and must be mounted explicitly. New stacks do not silently adopt old disks. A dump/restore is the simpler recovery path.

## Teardown

```sh
CONFIRM_DESTROY=poketeam aws/destroy.sh
```

For the standard us-east-2 deployment, this disables/deletes the distribution with its current ETag, then deletes regional compute, public IP/root disk, networking, role and budget. The **data disk, private bucket and SSM origin token remain**. Their IDs are recorded in `.state/<account>/<region>/<stack>/retained-*.json`. Retained storage still consumes credits/incurs storage usage. Failed creation can also leave retained resources; inspect CloudFormation before retrying or cleanup.

Only after verifying backups and the printed account/stack, permanently remove retained data:

```sh
CONFIRM_PURGE=poketeam:YOUR_12_DIGIT_ACCOUNT_ID aws/purge-data.sh
```

This refuses attached disks, checks bucket ownership/account, then deletes retained database data, backups and the origin token. Partial failures require inspection using the saved inventory. It does not clean unrelated account resources. `CONFIRM_DESTROY=poketeam aws/edge.sh destroy` can separately finish deletion of the standard edge if interrupted.

## Legacy deployments

Existing us-east-1 all-in-one CloudFormation deployments remain supported with their original FREE flat-rate subscription guard. They require an eligible account and permissions for WAF/pricing-plan-manager; they are not the default path for a Free managed project. The standard edge helper refuses saved subscription/WAF state and does not automatically migrate billing relationships. Do not switch an existing stack's region or pricing relationship by editing config alone; review its infrastructure and data first.

## Offline checks

```sh
python3 -m unittest discover -s aws/tests -v
python3 aws/template.py > /tmp/poketeam-stack.json
cfn-lint --regions us-east-1 --template /tmp/poketeam-stack.json
AWS_REGION=us-east-2 python3 aws/template.py > /tmp/poketeam-regional-stack.json
cfn-lint --regions us-east-2 --template /tmp/poketeam-regional-stack.json
for script in aws/*.sh aws/host/*.sh; do bash -n "$script"; done
```

Tests use a local fake CLI and temporary state. They verify creation/resume, ARM builds, ownership/secret handling, standard pricing without WAF/subscription calls, legacy subscription guards, traffic enablement and destructive cleanup confirmation. Mock success is separate from real account permissions and deployment health.

## Bootstrap recovery

The AL2023 package is named `awscli-2`. Compose checksum verification accepts the optional binary-mode `*` marker. Successful bootstrap records `/var/lib/poketeam/bootstrap-complete` only after disk mounting and Docker setup finish. After an explicitly repaired bootstrap, cloud-init can retain its historical error; deployment accepts that marker as completion evidence and still requires the data mount. It does not reset cloud-init history or ignore an unrepaired failure. The first live host required this recovery; the source bootstrap and nginx configuration were corrected. Nginx config validation in CI uses a full 64-character dummy secret to catch map-hash sizing errors.

## Current deployment

Public app: **https://d2d00yusoq9i0z.cloudfront.net**. Stack `poketeam` is in us-east-2; its instance runs in us-east-2a. Full catalog sync and public desktop/mobile API/UI checks passed. Daily backups are enabled and the initial encrypted S3 dump was verified. The AWS account remains FREE/ACTIVE. See [deployment evidence and recovery notes](../plans/aws-free-account-redesign.md).
