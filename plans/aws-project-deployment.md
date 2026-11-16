# Project-compatible AWS deployment

The user approved keeping regional infrastructure in us-east-2 CloudFormation and managing the global edge through the explicitly permitted CloudFront/WAF service APIs. Keep the existing us-east-1 all-in-one lifecycle compatible with older deployments; new us-east-2 deployments use the split lifecycle. This preserves the single host, encrypted retained data disk/private backup bucket, CloudFront hostname, disabled caching, origin secret, rate limit, and explicit FREE subscription.

## Implementation

Render a regional-only CloudFormation template for us-east-2. Manage global resources with a small standard-library Python helper calling AWS CLI with JSON files, so credentials remain owned by the CLI and secret headers never appear as command arguments. Keep private resource IDs in the existing account/region/stack state directory; deterministic names/caller references and API discovery allow interrupted creation to resume without duplicate resources. Validate ownership and the exact FREE subscription before enabling traffic. Preserve ETags/lock tokens for updates/deletes. Deployment, status and teardown must handle both lifecycles.

Run read-only edge API checks before any provisioning. Free subscription verification is mandatory: no fallback to pay-as-you-go, paid subscription, account upgrade or organization-policy edit. Current account denies pricingplanmanager:ListSubscriptions, so actual creation stays blocked while script preparation and offline tests continue. The user explicitly accepted this scope after the new denial was reported.

Teardown disables edge traffic, requests subscription cancellation, then deletes regional compute while retaining data as before. AWS schedules active subscription cancellation at the end of the billing period; retain disabled edge resources and their private IDs until the cancellation takes effect, then finish edge deletion explicitly. Do not imply immediate edge deletion or silently detach a Free plan into pay-as-you-go billing.

## Validation

Cover regional rendering, legacy compatibility, global API regions, idempotent/interrupted creation, secret handling, FREE subscription gates before resource creation and traffic enablement, ETag updates, and cancellation-aware teardown with a local fake CLI. Run shell syntax and CloudFormation lint checks. Record actual account checks and deployment limits separately from mocked tests.

## Results

Implemented us-east-2 regional template rendering, direct edge create/resume/enable/cleanup, combined status outputs, legacy us-east-1 compatibility, CI checks for both rendered templates, updated setup/teardown guidance and the gitignored us-east-2 config. The global helper preserves origin-header secrecy in private JSON request files, checks exact FREE resource bindings, uses CloudFront ETags/WAF lock tokens and retains disabled edge resources while subscription cancellation is pending.

All 21 offline AWS tests passed, covering both lifecycles, interrupted creation with lost local IDs, secret omission from command arguments, subscription permission failure before creation, paid/pending-change refusal, and cancellation-aware deletion. Both templates pass cfn-lint 1.60.1 for their intended regions; all shell scripts pass Bash syntax checks. The direct CloudFront create request also validates against the installed AWS CLI's botocore input model without a network request. The CLI's generated output skeleton has an unrelated OriginGroups minimum-length generation error, so validation used its actual input model instead of treating that generated response as evidence.

At the user's request to try the deployment, ran the updated preflight against the actual account. Regional EC2 offerings/AZs, public ARM AMI and CloudFront prefix-list lookups passed, as did global WAF and CloudFront list calls. The mandatory pricing-plan-manager ListSubscriptions call was explicitly denied by organization policy p-323vgvzk. Full output is captured at logs/aws-us-east-2-preflight.log. No token, stack, instance, bucket, WAF, distribution or subscription was created. Deployment remains blocked on an account/project permitting the pricingplanmanager service; no paid fallback, account upgrade or policy edit was attempted. Changes are prepared but uncommitted. The unrelated reqwest-probe deletion remains untouched.
