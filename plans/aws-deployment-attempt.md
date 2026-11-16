# AWS deployment attempt — 2026-11-16

The user requested setting up the existing AWS deployment using their logged-in CLI and supplied the budget-alert email. Use the previously reviewed single-host deployment scripts; no architecture or account-plan changes were made.

## Preparation and checks

Prepared gitignored `aws/config.sh` with the default CLI profile, `us-east-1`, stack `poketeam`, the user's budget email, a $5 monthly budget alert, and the existing small encrypted-volume defaults. The file is mode 0600. `ALLOW_PAID_RESOURCES` remains disabled. AWS CLI 2.40.3 and Docker 29.9.3 are available; the active Docker Desktop builder supports linux/arm64. All 10 offline AWS deployment tests passed.

The authenticated account is `030190455630`. Read-only preflight reports an ACTIVE FREE account plan with $120 shared credits expiring 2027-05-13. This confirms account credits, not that the deployment has zero cost.

## Deployment blocker

`aws/preflight.sh` stopped at `ec2:DescribeInstanceTypeOfferings` in `us-east-1` with an explicit organization service-control-policy deny. AWS's `iam-toolbox get-request-authorization-details` confirmed policy `arn:aws:organizations::872845099240:policy/o-wrcxt9s21w/service_control_policy/p-323vgvzk`, statement `UsEast1Partitional`, denies the request despite the role's AdministratorAccess allow. Reading that policy via Organizations was also denied. These read-only diagnostics did not change policies or resources.

No CloudFormation stack, EC2 instance, bucket, subscription, budget, or origin token was created. No account upgrade, region switch or policy change was attempted. The unrelated local deletion of the reqwest diagnostic binary remains untouched.

## Next step

The account/organization administrator must confirm an approved deployment region/identity or adjust the policy for the existing us-east-1 deployment. The current scripts deliberately support only us-east-1 because they combine regional resources with global CloudFront/WAF resources. Adapting to another approved region requires reviewing that split before provisioning. Once access is resolved, rerun `aws/preflight.sh`, then follow create/backend/frontend deployment and live verification in `aws/README.md`. An existing authenticated CLI session alone cannot override an explicit organization deny.

## Requested us-east-2 check

The user requested trying us-east-2. Read-only EC2 instance-type offerings succeeded for t4g.small in us-east-2a/b/c. The public AL2023 ARM image parameter resolved successfully. CloudFormation DescribeStacks was allowed and reported no existing `poketeam` stack in that region.

The existing all-in-one template cannot simply move to us-east-2: AWS requires CloudFront-scope WAF resources in us-east-1 (https://docs.aws.amazon.com/AWSCloudFormation/latest/TemplateReference/aws-resource-wafv2-webacl.html). A read-only DescribeStacks check for a potential `poketeam-edge` stack in us-east-1 was explicitly denied by the same organization policy p-323vgvzk. Thus splitting the regional host into us-east-2 and the edge into a us-east-1 CloudFormation stack still needs organization permission for the latter. No resource creation, policy bypass, paid-plan change or partial deployment was attempted. The scripts/config remain unchanged pending a permitted edge provisioning path; implementing a two-stack lifecycle while its required region is denied would not unblock deployment.

## Managed-project policy clarification

Further primary-source research identified AWS's managed project policy, which uses the same `UsEast1Partitional` statement name: https://docs.aws.amazon.com/accounts/latest/reference/scps-and-rcps-for-projects.html . Its documented exceptions explicitly include CloudFront and WAFv2 in us-east-1, while excluding CloudFormation. Therefore the earlier statement that an administrator must enable a us-east-1 edge CloudFormation stack was too restrictive: an explicitly permitted service-API provisioning path may support this project without changing its organization policy.

A read-only `aws wafv2 list-web-acls --scope CLOUDFRONT --region us-east-1 --limit 1` succeeded (zero ACLs returned), confirming direct global WAF access. This is distinct from the denied CloudFormation service; no policies or resources were changed. A project-compatible deployment would keep regional infrastructure in us-east-2 CloudFormation and manage the global edge through explicitly permitted APIs. The existing scripts require adaptation, lifecycle tests and Free-plan permission validation before such a deployment; no claim of successful provisioning is made.
