#!/usr/bin/env python3
"""Small local helpers for JSON and safe shell quoting; AWS operations stay in CLI scripts."""
import datetime as dt
import json
import os
from pathlib import Path
import secrets
import shlex
import sys


def read(path):
    return json.loads(Path(path).read_text())


def write(path, value):
    Path(path).write_text(json.dumps(value, indent=2) + "\n")


def instance_type(today=None):
    today = today or dt.datetime.now(dt.timezone.utc).date()
    return os.environ.get("INSTANCE_TYPE") or ("t4g.small" if today <= dt.date(2026, 12, 31) else "t4g.micro")


def preflight(identity, plan, state):
    choice = instance_type()
    if choice not in ("t4g.micro", "t4g.small"):
        raise ValueError("INSTANCE_TYPE must be t4g.micro or t4g.small (ARM64).")
    credits = plan.get("accountPlanRemainingCredits", {}).get("amount", 0)
    print(f"Account: {identity['Account']} | Region: us-east-1 | Instance: {choice}")
    print(f"Free Tier plan: {plan.get('accountPlanType', 'unknown')} / {plan.get('accountPlanStatus', 'unknown')}")
    print(f"Remaining shared credits: ${credits}; expires: {plan.get('accountPlanExpirationDate', 'unknown')}")
    print("t4g.small's 750-hour/month trial is documented through 2026-12-31; existing usage shares that allowance.")
    print("Storage, IPv4, S3 requests and usage outside allowances can consume credits or incur charges.")
    print("One public IPv4 is ~$3.65/month at $0.005/hour; 24 GiB gp3 is ~$1.92/month at $0.08/GiB.")
    print("These are illustrative list prices, not a bill estimate or guarantee of free operation.")
    if (plan.get("accountPlanStatus") != "ACTIVE" or credits <= 0) and os.environ.get("ALLOW_PAID_RESOURCES") != "1":
        raise ValueError("Cannot verify active credits. Review Billing, then set ALLOW_PAID_RESOURCES=1 only if you accept uncovered charges.")
    Path(state, "instance-type").write_text(choice + "\n")


def parameters(state, enabled):
    state = Path(state)
    offerings = {o["Location"] for o in read(state / "offerings.json")["InstanceTypeOfferings"]}
    zones = [z for z in read(state / "zones.json")["AvailabilityZones"] if z["ZoneName"] in offerings]
    if not zones:
        raise ValueError("No available availability zones.")
    prefix = (state / "prefix-list-id").read_text().strip()
    if not prefix.startswith("pl-"):
        raise ValueError("CloudFront origin-facing prefix list was not found.")
    values = {
        "AmiId": (state / "ami-id").read_text().strip(),
        "InstanceType": (state / "instance-type").read_text().strip(),
        "AvailabilityZone": zones[0]["ZoneName"],
        "CloudFrontPrefixList": prefix,
        "OriginToken": read(state / "token.json")["Parameter"]["Value"],
        "EnableTraffic": enabled,
        "BudgetEmail": os.environ["BUDGET_EMAIL"],
        "MonthlyBudgetUsd": os.environ.get("MONTHLY_BUDGET_USD", "5"),
        "RootGiB": os.environ.get("ROOT_GIB", "16"),
        "DataGiB": os.environ.get("DATA_GIB", "8"),
    }
    for key in ("RootGiB", "DataGiB"):
        if not 8 <= int(values[key]) <= 30:
            raise ValueError(f"{key} must be 8–30 GiB.")
    write(state / "parameters.json", [f"{key}={value}" for key, value in values.items()])


def deploy_command(path, bucket, release, stack):
    # Values come from AWS outputs/generated identifiers, but still quote every shell argument.
    script = f"""#!/bin/bash
set -euo pipefail
cloud-init status --wait
mountpoint -q /srv/poketeam
install -d -m 700 /srv/poketeam/releases/{shlex.quote(release)}
cd /srv/poketeam/releases/{shlex.quote(release)}
aws s3 cp {shlex.quote(f's3://{bucket}/releases/{release}.tar.gz')} release.tar.gz --only-show-errors
tar -xzf release.tar.gz
aws ssm get-parameter --name {shlex.quote('/' + stack + '/origin-token')} --with-decryption --query Parameter.Value --output text > /srv/poketeam/origin-token
chmod 600 /srv/poketeam/origin-token
bash host/activate.sh
"""
    Path(path).write_text(script)


def frontend_command(path, bucket, release):
    script = f"""#!/bin/bash
set -euo pipefail
mountpoint -q /srv/poketeam
exec 9>/srv/poketeam/deploy.lock
flock -n 9
[[ -f /srv/poketeam/current/compose.yaml ]] || {{ echo 'Deploy the backend first.' >&2; exit 1; }}
install -d -m 755 /srv/poketeam/public/releases/{shlex.quote(release)}
cd /srv/poketeam/public/releases/{shlex.quote(release)}
aws s3 cp {shlex.quote(f's3://{bucket}/releases/{release}.tar.gz')} /srv/poketeam/frontend-upload.tar.gz --only-show-errors
tar -xzf /srv/poketeam/frontend-upload.tar.gz
chmod -R a+rX .
[[ -f index.html ]]
# Mount the parent directory in nginx so replacing this link is visible without recreating it.
ln -sfn {shlex.quote('releases/' + release)} /srv/poketeam/public/next
mv -Tf /srv/poketeam/public/next /srv/poketeam/public/current
rm /srv/poketeam/frontend-upload.tar.gz
"""
    Path(path).write_text(script)


def enable_parameters(state):
    state = Path(state)
    body = read(state / "current-template.json")["TemplateBody"]
    if isinstance(body, str):
        body = json.loads(body)
    write(state / "enabled-template.json", body)
    # AWS CLI deploy automatically keeps previous values for omitted parameters, including NoEcho.
    write(state / "enable-parameters.json", ["EnableTraffic=true", "OriginDomainName=" + (state / "origin-dns").read_text().strip()])


def main():
    action, *args = sys.argv[1:]
    if action == "preflight":
        preflight(read(args[0]), read(args[1]), Path(args[0]).parent)
    elif action == "parameters":
        parameters(*args)
    elif action == "new-token":
        write(args[0], {"Name": f"/{os.environ.get('STACK_NAME', 'poketeam')}/origin-token", "Type": "SecureString", "Value": secrets.token_hex(32), "Tags": [{"Key": "Project", "Value": os.environ.get("STACK_NAME", "poketeam")}]})
    elif action == "deploy-command":
        deploy_command(*args)
    elif action == "frontend-command":
        frontend_command(*args)
    elif action == "enable-parameters":
        enable_parameters(*args)
    elif action == "validate-frontend":
        for path in Path(args[0]).rglob("*"):
            if path.is_symlink() or not (path.is_file() or path.is_dir()):
                raise ValueError(f"Static build contains a symlink or special file: {path}")
    elif action == "retained-values":
        state, account = Path(args[0]), args[1]
        if read(state / "identity.json")["Account"] != account:
            raise ValueError("Current AWS account does not match the provisioning account.")
        resources = read(state / "retained-logical-resources.json")["StackResourceSummaries"]
        values = {r["LogicalResourceId"]:r.get("PhysicalResourceId", "") for r in resources}
        write(state / "retained-data.json", {"volume": values.get("DataVolume", ""), "bucket": values.get("Bucket", "")})
    elif action == "check-offerings":
        if not read(args[0])["InstanceTypeOfferings"]:
            raise ValueError("Selected instance type is not offered in this region.")
    elif action == "check-stack":
        status = read(args[0])["Stacks"][0]["StackStatus"]
        if status not in ("CREATE_COMPLETE", "UPDATE_COMPLETE"):
            raise ValueError(f"Stack is {status}; inspect CloudFormation events before continuing.")
    elif action == "print-outputs":
        for item in read(args[0])["Stacks"][0].get("Outputs", []):
            print(f"{item['OutputKey']}: {item['OutputValue']}")
    elif action == "print-invocation":
        invocation = read(args[0])
        print(invocation.get("StandardOutputContent", ""))
        print(invocation.get("StandardErrorContent", ""), file=sys.stderr)
    else:
        raise ValueError(f"Unknown action: {action}")


if __name__ == "__main__":
    try:
        main()
    except (ValueError, KeyError) as error:
        sys.exit(str(error))
