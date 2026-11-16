# Low-cost AWS hosting

These AWS CLI scripts provision a small, single-host deployment of the existing backend and the React/static frontend. **Nothing is deployed by adding this directory.** Start by reviewing [the deployment decisions](../plans/aws-hosting.md) and [stack.template.json](stack.template.json). Scripts support **us-east-1** initially, placing the global WAF/CloudFront resources and regional resources in one stack.

## Cost and Free Tier

AWS changed its account Free Tier on July 15, 2025: newer accounts receive credits and a Free/Paid account plan, rather than an unconditional year of free EC2/RDS. `preflight.sh` reads your actual account plan, remaining shared credits, expiry, ARM AMI, and available instance locations. It fails before resource creation if it cannot verify active credits, unless you explicitly set `ALLOW_PAID_RESOURCES=1`. An active credit balance is not a guarantee that this application or future usage stays free. The scripts never upgrade your account plan. See [AWS Free Tier](https://aws.amazon.com/free/) and [account-plan API](https://docs.aws.amazon.com/cli/latest/reference/freetier/get-account-plan-state.html).

The default instance is **t4g.small (2 GiB)** while its documented 750-hour/month trial lasts, currently through **December 31, 2026**. That allowance is shared across regions/instances, and the trial does not cover attached storage or IP charges. New deployments default to **t4g.micro (1 GiB)** afterward; swap and container memory limits support light usage, with less headroom. Existing instances are never automatically resized or terminated at the trial deadline. See [AWS’s T4g trial FAQ](https://aws.amazon.com/ec2/faqs/) and [EC2 eligibility](https://docs.aws.amazon.com/AWSEC2/latest/UserGuide/ec2-free-tier-usage.html). Review pricing again before January 2027.

| Resource | Chosen cost control |
| --- | --- |
| EC2 | One ARM instance; Standard CPU credits prevent Unlimited surplus-credit fees; image built locally |
| EBS | 16 GiB root + 8 GiB data, encrypted gp3, baseline performance |
| Public IPv4 | One auto-assigned address; no additional Elastic IP |
| CloudFront/WAF | Explicit **FREE** flat-rate subscription; traffic starts disabled and is enabled only after verifying the live subscription and deploying the application |
| S3 | One private encrypted artifact/backup bucket; release objects expire after 7 days, database dumps after 14 days; incomplete multipart uploads expire after 1 day |
| Administration | SSM, no SSH port, key pair, or bastion |
| Database | PostgreSQL on the same host; no separate RDS instance |
| Other | No ALB, NAT gateway, ECS cluster, ECR repository, hosted DNS zone, domain purchase, paid logs, or snapshots |

Before credits/allowances, one IPv4 is approximately **$3.65/month** at 730 hours × $0.005/hour; 24 GiB gp3 is approximately **$1.92/month** at $0.08/GiB-month. Compute after the trial, S3 requests/storage, and applicable transfer/taxes are additional. These illustrative rates are not an account-specific quote. See [IPv4 prices](https://aws.amazon.com/vpc/pricing/) and [EBS prices](https://aws.amazon.com/ebs/general-purpose/).

The CloudFront Free plan currently includes a 100 GB/month and 1M-request baseline allowance, plus S3 storage credits. The template explicitly subscribes to that plan with its WAF ACL; a pay-as-you-go distribution alone is different. Allowances are not a hard request cap; AWS can constrain excessive sustained usage. Features and eligibility may change. See [plan features and limits](https://docs.aws.amazon.com/AmazonCloudFront/latest/DeveloperGuide/flat-rate-pricing-plan.html). The account-wide budget defaults to a $5/month threshold, excluding credits/refunds to show underlying spend, with an email alert above 80%. **A budget alert does not cap spending or stop resources.** Other workloads in your account also count.

## Architecture and limits

```text
Browser ── HTTPS ── CloudFront (Free plan + rate-limit WAF)
                        │ HTTP, CloudFront address ranges + secret header
                        ▼
                 EC2 nginx :80
                  ├─ /api/*, /health/*, /openapi.json → API :3000
                  └─ /* → static frontend files / SPA fallback
                        │
                  PostgreSQL + worker
                        │
                 retained encrypted EBS
                        └─ daily pg_dump → private S3 (14-day expiry)
```

CloudFront supplies an AWS-managed HTTPS hostname without a domain purchase. The backend receives that HTTPS origin, uses Secure HttpOnly cookies, and validates browser Origin headers. CloudFront forwards cookies and Origin with **caching disabled** for the whole distribution, including error caching; no session/team response can be shared from cache. Frontend files also start uncached to keep the first deployment simple. SPA fallback applies only to frontend paths, never API errors.

The **CloudFront-to-EC2 hop uses HTTP**, restricted by the managed CloudFront prefix list and a random 256-bit origin header. This is not end-to-end application-layer TLS. The Free plan excludes VPC origins; using a custom origin domain/certificate for HTTPS or a different plan is a later reviewed change. Do not treat this starter deployment as a high-availability production design. A host/AZ failure interrupts both application and database; recovery uses the retained disk or logical backups. Raw API port 3000 is loopback-only, PostgreSQL is not published, and port 22 is closed. The instance requires IMDSv2; app containers cannot access its role through the default one-hop metadata limit.

## First deployment

Install current **AWS CLI v2** (including `freetier get-account-plan-state` and `pricing-plan-manager`), Python 3, Bash, and Docker with buildx. Configure an AWS profile/SSO session with permission to manage this stack: CloudFormation, EC2/VPC/EBS, IAM roles/instance profiles and PassRole, S3, SSM/KMS for the origin parameter, CloudFront, WAFv2, PricingPlanManager, Budgets, plus read access to STS/Free Tier/public AMI parameters. Use a normal administrative deployment identity, not root credentials; no access keys are embedded in scripts or containers. The instance role is created separately with SSM administration and scoped S3 release/backup access. `AmazonSSMManagedInstanceCore` also grants SSM parameter-read operations; the token uses the account's AWS-managed SSM encryption key.

```sh
cp aws/config.example.sh aws/config.sh
# Edit profile, budget email, stack name, and any instance/storage overrides.
aws sso login --profile your-profile    # If using an SSO profile
aws/preflight.sh                      # Read-only AWS queries
aws/create.sh                         # Creates infrastructure; public traffic remains disabled
aws/deploy-backend.sh                  # Builds linux/arm64, uploads privately, migrates, starts services
aws/status.sh
```

`create.sh` is repeatable for an existing healthy stack and does not replace it or rotate the origin secret. Resolve failed/rolled-back CloudFormation stacks explicitly before retrying. CloudFront global deployment, instance bootstrapping, and the initial catalog sync take time. The script uses SSM to wait for cloud-init and mounts the correct EBS volume by serial number. Docker cannot start at boot unless the data filesystem is mounted, preventing accidental initialization of a second empty database on the root disk.

The first application deployment generates a random database password **on the instance**, stored only in its encrypted retained data volume. The CloudFront origin token is generated once into an SSM SecureString and passed to CloudFormation via a private NoEcho parameter file. It is fetched on-host rather than embedded in SSM command history. Local generated files under `aws/.state/<account>/<region>/<stack>/` and `aws/config.sh` are excluded from Git and Docker build contexts; keep them private. No AWS credential files are read or copied by these scripts.

After deployment, use the printed origin:

```sh
curl https://YOUR_DISTRIBUTION.cloudfront.net/health/live
curl https://YOUR_DISTRIBUTION.cloudfront.net/health/ready
curl https://YOUR_DISTRIBUTION.cloudfront.net/api/v1/catalog/status
curl -i -X POST https://YOUR_DISTRIBUTION.cloudfront.net/api/v1/session \
  -H 'Origin: https://YOUR_DISTRIBUTION.cloudfront.net' \
  -H 'Content-Type: application/json' -d '{}'
```

Readiness is 503 until the worker publishes its initial complete catalog. It retries upstream failures on the backend's persisted schedule. Liveness only indicates a running API process. Check SSM/CloudFront/WAF state if browser requests fail; the rate rule limits each source IP to approximately 1,000 requests per five-minute window. Many users sharing an IP also share that limit.

## Deploy the frontend

Build the frontend locally (`cd frontend && npm ci && npm run build`) with relative `/api/v1/...` URLs, then upload the static output from the repository root:

```sh
aws/deploy-frontend.sh frontend/dist
```

The directory must contain `index.html` and no symlinks or special files. Uploads go through private S3 and SSM; switching the frontend directory link is atomic, and both frontend and API keep the same CloudFront origin. No frontend code, build pipeline, Node server, or extra AWS resources are required by this script. Rendered frontend files are public by design; never put secrets in the build directory.

## Updates, logs, backups, recovery

Run `aws/deploy-backend.sh` again to publish another backend image. It takes a deployment lock, stops API/worker/proxy writes on existing deployments, backs up the database, migrates, and recreates the services. Expect brief downtime. A migration/startup failure stops the script, leaves traffic unavailable, and requires inspection; it does not claim a successful rollback or downgrade the database. Current/previous release configuration and application images remain available. Confirm schema compatibility before rolling code back; restore a database backup when required. Root-disk Docker logs rotate at 3 × 10 MB per service, and old application images are removed except current/previous or images still in use. Downloaded backend archives are removed after successful deployment. Static frontend releases remain on the small data disk; remove obsolete ones through SSM when no longer needed.

`aws/status.sh` prints an SSM session command. Install the Session Manager plugin to use it, then inspect:

```sh
sudo tail -n 100 /var/log/poketeam-bootstrap.log
sudo cloud-init status --long
sudo docker compose --env-file /srv/poketeam/runtime.env \
  -f /srv/poketeam/current/compose.yaml logs --tail 100 api worker
sudo systemctl status poketeam-backup.timer poketeam-backup.service
```

`aws/backup-now.sh` requests an immediate logical backup via SSM. The daily systemd timer runs around 04:00 UTC and catches missed schedules. Inspect the service's last status and S3 objects to verify backups; this starter does not add a paid monitoring/alerting service for backup failures. Dumps contain team ownership/history/sprites and should stay private. The bucket lifecycle **continues expiring backups after stack deletion**, so download any long-term recovery copies before the 14-day retention ends.

To restore a dump, download it onto a private trusted host or through SSM, use `pg_restore --exit-on-error` into a **new empty PostgreSQL database**, and verify catalog/teams before switching the app's database URL. See [backend backup/restore guidance](../docs/operations.md). A retained EBS disk also contains `/srv/poketeam/runtime.env` and the full database. Moving it requires the same AZ; mount it explicitly and verify the host role's bucket permissions. `create.sh` intentionally does not automatically adopt/overwrite a disk from a deleted stack. Restoring a dump into a newly created stack is the simpler recovery path.

Stopping/restarting EC2 can change its public IPv4/DNS name. The CloudFront viewer hostname remains fixed; rerun `deploy-backend.sh` after starting the host to refresh CloudFront's origin DNS. A successful app deployment updates only traffic/DNS parameters and keeps the previous AMI, secret, and volume configuration. Infrastructure resizing/replacement is a deliberate manual migration; the scripts do not silently move or reformat a live database disk. Keep ordinary OS/package/container security updates in your maintenance routine.

## Teardown and remaining charges

```sh
CONFIRM_DESTROY=poketeam aws/destroy.sh
```

This deletes compute, its auto-assigned public IP/root disk, CloudFront subscription/distribution/WAF, networking, role, and budget. CloudFormation cancels the Free subscription before deleting the distribution. **The data disk, S3 bucket, and origin-token parameter remain**, with IDs recorded in `aws/.state/<account>/us-east-1/poketeam/retained-*.json`. Retained storage keeps incurring charges even though the application is gone. Failed initial stack creation can also leave retained storage; inspect its CloudFormation resources and run teardown.

To permanently remove those retained resources, after verifying your own backups and the printed account/stack:

```sh
CONFIRM_PURGE=poketeam:YOUR_12_DIGIT_ACCOUNT_ID aws/purge-data.sh
```

This requires the saved resource inventory and matching AWS account, refuses an attached data volume, empties/deletes the bucket, deletes the detached database disk, and deletes the origin token. **All database contents and bucket backups are permanently lost.** If a purge partially fails, use the saved IDs to inspect and finish cleanup explicitly; it is not a blanket account cleanup tool. Domain/account-wide resources not created by this stack are untouched.

## Offline validation

```sh
python3 -m unittest discover -s aws/tests -v
python3 aws/template.py > /tmp/poketeam-stack.json
cfn-lint --regions us-east-1 --template /tmp/poketeam-stack.json
for script in aws/*.sh aws/host/*.sh; do bash -n "$script"; done
```

The tests use a local fake `aws`/`docker` CLI and temporary state, exercising eligibility guards, trial expiry, AZ selection, private token handling, repeatable creation, ARM image intent, subscription checks, frontend packaging, and destructive cleanup guards. No AWS resources or credentials are used. Live deployment validation requires your configured account and is not claimed by these offline checks.
