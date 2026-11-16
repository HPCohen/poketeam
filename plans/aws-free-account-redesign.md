# Public hosting while keeping the AWS account on Free

Status: user approved implementation and deployment of the recommended standard-pricing CloudFront/EC2 design. This supersedes the requirement for a CloudFront flat-rate FREE subscription. The user explicitly wants to avoid upgrading the AWS account to Paid or activating advanced features.

## Findings

AWS's managed-project service list includes EC2, CloudFront, API Gateway and Lambda on Free accounts:
https://docs.aws.amazon.com/accounts/latest/reference/supported-services-sign-up-new.html

The account restriction is on CloudFront flat-rate plans, including their $0 tier:
https://docs.aws.amazon.com/PricingPlanManager/latest/UserGuide/plans.html

Standard usage-based CloudFront is a different service-pricing choice from an AWS Paid account. AWS documents its usage-based pricing and monthly free allowances separately:
https://aws.amazon.com/cloudfront/faqs/

Therefore the current eligibility blocker does not require changing databases or runtimes. The supported service list and pricing documentation support using a standard distribution without the flat-rate subscription; actual creation still requires a successful deployment in this specific project. Prior live checks already verified CloudFront read access and regional EC2 access, not creation permissions.

## Recommended design

Browser → HTTPS CloudFront standard distribution → HTTP nginx on EC2 in us-east-2 → existing Rust API / private PostgreSQL. Nginx serves the built React frontend from the same origin. The existing sync worker continues as a separate container on the same host.

Use the assigned cloudfront.net hostname and certificate; no purchased domain or certificate service is needed. Keep caching disabled for the initial deployment, forward cookies and Origin, preserve exact same-origin validation and Secure cookies, and keep API/PostgreSQL ports private. Keep the CloudFront-managed prefix-list ingress restriction and the random secret header between CloudFront and nginx. The HTTP origin hop and single-host failure domain are the same reviewed tradeoffs as the original design.

Remove WAF and all pricing-plan-manager/subscription calls from this deployment mode. Add modest nginx request limits as local protection, while explicitly recognizing they are not equivalent to managed WAF or an edge billing cap. Standard CloudFront has usage-based pricing and does not provide the flat-rate plan's no-overage guarantee. Existing AWS Free account credits and expiry limits still apply; EC2, volumes, public IPv4 and backups can consume credits. Avoid claiming permanently free hosting or automatic conversion to a Paid account.

This is the smallest change to the tested application: no Rust handlers, database schema, sync logic, ownership model or frontend API contract need changing. Retain encrypted EBS, private S3 releases/backups, SSM administration, stable viewer URL and a single-origin app. Deploy the regional stack and standard distribution, build/migrate/start backend and worker, deploy frontend, then enable traffic after liveness checks. Check readiness/full-sync status separately. No flat-rate subscription needs to be verified or canceled; teardown can disable and delete the distribution directly using its current ETag.

Implementation should replace the unpublished flat-rate-only edge mode with an explicit, documented standard-pricing mode, remove obsolete Free subscription guards from that mode, and update offline lifecycle/secret/traffic/teardown tests. Preserve safeguards for any legacy subscription-backed deployment rather than silently changing an existing billing relationship. No existing cloud deployment was created in prior attempts.

## Alternatives

1. Direct EC2 public IP/DNS: the fewest services. Serve the existing nginx frontend/API through an internet-routed public IPv4 and port 80; remove the CloudFront-only ingress and header requirements in that explicit mode and set PUBLIC_ORIGIN to the actual address. Raw HTTP does not encrypt the team's identity cookie, so it is suitable only for a temporary inspection. A domain and HTTPS termination on-host would be the normal durable configuration. Auto-assigned IPv4 can change on stop/start. No Lambda, ALB or subscription is required.
2. API Gateway HTTP API → EC2: a regional AWS-managed HTTPS endpoint can proxy a publicly routable HTTP backend. This can serve the frontend and API through one hostname without a Lambda function. It adds per-request service usage and HTTP API timeout/payload constraints. Protect the EC2 origin with an injected secret header; API Gateway does not provide the same CloudFront-only source-prefix-list restriction. The app/runtime/database remain the same. It is a reasonable alternative if avoiding CloudFront entirely is a requirement.
3. Lambda function URL: supported public HTTPS endpoint using NONE authentication plus public invocation permissions, but the Rust process needs a Lambda integration, PostgreSQL still needs durable hosting, and the long-running sync worker becomes scheduled bounded invocations (standard event invocation maximum is 15 minutes). It does not solve the persistent database requirement. For the current app it increases changes without removing the database host; a broader serverless data-model redesign should be discussed separately.

Sources for alternatives:
https://docs.aws.amazon.com/accounts/latest/reference/sign-up-for-aws.html
https://docs.aws.amazon.com/AWSEC2/latest/UserGuide/using-instance-addressing.html
https://docs.aws.amazon.com/apigateway/latest/developerguide/http-api-develop-integrations-http.html
https://docs.aws.amazon.com/lambda/latest/dg/urls-auth.html
https://docs.aws.amazon.com/lambda/latest/dg/gettingstarted-limits.html

## Decision boundary

Discuss the standard-pricing / no-managed-WAF tradeoff with the user before replacing the subscription-backed deployment path or exposing any origin directly. The recommended redesign preserves the Free account and the existing application. No account upgrade, advanced-feature activation, policy change, domain purchase, database migration or resource creation is authorized by this design document itself.

## One-week light-traffic estimate — 2026-11-16

Assume 168 hours in us-east-2 on t4g.small/Linux, Standard CPU credits, 16 GB root plus 8 GB data at gp3 baseline performance, one public IPv4, small private S3 releases/daily backups, approximately 100 visits/day, 14,000 web requests and 2 GB CloudFront viewer transfer in the week. No WAF, NAT gateway, load balancer, RDS, paid logs or flat-rate subscription. CloudFront's standard free allowance is assumed available to this account/organization.

The AWS Price List API returned Linux/shared t4g.small/us-east-2 at $0.0168/hour (query saved in ignored logs/aws-ec2-weekly-price.json), or $2.8224/week. IPv4 at $0.005/hour is $0.84/week. 24 GB gp3 at $0.08/GB-month prorated over a 30-day month is $0.448/week. Allow roughly $0.01–$0.05 for small S3 storage/requests; CloudFront viewer transfer/requests fit its standard monthly free allowance. Estimate about $4.15/week at normal compute rates before account credits/tax, with $5 a reasonable planning allowance.

AWS documents an automatically enrolled 750 aggregate hour/month t4g.small trial through 2026-12-31, including Ohio. If its allowance is unused and applies as documented, subtract $2.82 compute, leaving about $1.30–$1.35/week before account credits. Do not treat the trial as permanent or assume another workload has not consumed its shared allowance. The last actual account check reported $120 active credits, which should cover this small eligible usage while the account remains within its Free plan limits. This is a planning estimate, not an invoice or evidence that hosting has been provisioned.

Pricing sources:
https://aws.amazon.com/ec2/faqs/
https://aws.amazon.com/vpc/pricing/
https://aws.amazon.com/ebs/pricing/
https://aws.amazon.com/cloudfront/faqs/
https://aws.amazon.com/s3/pricing/

## Implementation and deployment

The user approved setup after reviewing the weekly estimate. The us-east-2 lifecycle now creates a standard CloudFront distribution without WAF or a flat-rate subscription. It refuses preexisting subscription/WAF state rather than changing a billing relationship. Regional deployment retains the same volumes, backups, origin-header/ingress restrictions and application containers. Standard edge validation checks the saved pricing choice, owned caller reference and absent WAF. Nginx applies an aggregate 20 API requests/second with a burst of 40; it is local protection, not an edge WAF or cost cap. Legacy us-east-1 subscription-backed scripts remain guarded separately. Account plan upgrades remain outside authorization. Automated and live deployment outcomes will be recorded below.

### Live provisioning and fixes

Standard-pricing preflight passed on the existing FREE/ACTIVE account with $120 credits. Created the us-east-2 `poketeam` regional stack, EC2 host `i-0689807f38ffd691a`, retained data volume `vol-0a182bfa87b51da19`, private bucket `poketeam-bucket-fx3flecxgsyp`, and disabled CloudFront distribution `E2O4I2XTGN0Q9N` at https://d2d00yusoq9i0z.cloudfront.net. Creation did not call pricing-plan-manager or create WAF resources.

The first real bootstrap exposed the incorrect `awscli2` package name; AL2023 requires `awscli-2`. The next attempt exposed Docker checksums using a binary-mode `*` filename marker. Fixed both source issues and explicitly reran the guarded bootstrap on the new host through SSM. The intended empty data disk was initialized/mounted and Docker started. The source now records a completion marker only after bootstrap succeeds; activation can accept this marker when cloud-init retains the historical first-attempt error, while still requiring the data mount. Cloud-init history was not erased. An initial local diagnosis used the wrong shell and therefore the wrong region; it was denied, made no cloud changes, and its accidental local scratch files were removed before retrying through Bash in us-east-2.

Backend migration/API/worker startup then exposed nginx's default map hash bucket being too small for the 64-character origin secret. Set map_hash_bucket_size to 128 and verified nginx -t with a full-length dummy secret against the actual nginx image; added that check to CI. The subsequent normal backend deployment passed API and proxy liveness, rejected requests without the origin header, installed the backup timer and enabled CloudFront. Distribution propagation and frontend/public verification follow below.

Checks so far: 17 AWS lifecycle tests pass; both CloudFormation renders pass linting; shell syntax and git whitespace checks pass; the ARM release image builds; frontend type/API-drift/build checks pass. Captured logs: logs/aws-standard-preflight.log, logs/aws-standard-create.log, logs/aws-image-build.log, logs/aws-standard-backend*.log. Generated state remains private/ignored. No account upgrade, advanced-feature activation, domain purchase or database-model change occurred.

### Completed public deployment

The app is live at https://d2d00yusoq9i0z.cloudfront.net using standard CloudFront pricing and EC2 in us-east-2a. Backend activation and frontend upload completed successfully. The public liveness/readiness endpoints return 200. The initial cloud sync completed all 1,025 Pokémon successfully on 2026-11-16 at 14:52:25 UTC (zero change alerts on initial import), and all nine generations/93 starter-family entries are available. The cloud database is a fresh deployment; no local identities/teams were copied.

Live checks through CloudFront passed: exact-Origin rejection, session creation, Secure/HttpOnly cookies, team create/read/update/delete, revision conflicts, power counter generation, independent sprite choices and retention/reset behavior, full variant metadata, combined type/generation/group filters, Legendary/Mythical separation, pagination and validation. Temporary test teams were deleted. Real desktop Chromium saved/reloaded a shiny Bulbasaur roster and loaded the selected image; mobile Chromium verified combined filters and no horizontal overflow. Screenshots are at /tmp/poketeam-aws-desktop.png and /tmp/poketeam-aws-mobile.png (local review artifacts).

Daily backup timer installation succeeded. Triggered and verified the first private AES256-encrypted S3 database dump at backups/2026-11-16T15-34-58Z.dump (1,773,308 bytes). EC2 instance/system/attached-disk status checks pass and SSM is Online. A final Free Tier API check still reports FREE / ACTIVE and $120 credits; billing consumption may appear later. No account-plan/advanced-feature change, WAF or flat-rate subscription was made.

Relevant rollout logs: logs/aws-standard-backend-final.log, logs/aws-standard-frontend.log and logs/aws-standard-backup.log. The successful backend release is 20261116T152614Z-3b6b6f38. Changes remain uncommitted; the unrelated user deletion of backend/src/bin/reqwest-probe.rs is untouched. The historical failed cloud-init status remains documented; the explicit guarded repair completed and the host/app services are healthy.
