#!/usr/bin/env python3
"""Offline AWS/Docker double: records intent, never imports boto or accesses the network."""
import json
import os
from pathlib import Path
import sys

root = Path(os.environ["FAKE_STATE"])
root.mkdir(exist_ok=True)
args = sys.argv[1:]
with (root / "calls.jsonl").open("a") as log:
    log.write(json.dumps([Path(sys.argv[0]).name, *args]) + "\n")


def arg(flag):
    return args[args.index(flag) + 1]


def emit(value):
    print(json.dumps(value))


def fail(message):
    sys.exit(message)


def stack():
    return {"Stacks": [{"StackStatus": "CREATE_COMPLETE", "Outputs": [
        {"OutputKey": k, "OutputValue": v} for k,v in {
            "InstanceId": "i-0123456789abcdef0", "BucketName": "mock-private-bucket", "DataVolumeId": "vol-0123456789abcdef0", "DistributionId": "DIST123",
            "SubscriptionStatus": "ACTIVE", "SubscriptionArn": "arn:aws:pricingplanmanager::123456789012:subscription:example", "PublicOrigin": "https://example.cloudfront.net",
        }.items()]}]}


if Path(sys.argv[0]).name == "docker":
    if args[0] == "save":
        sys.stdout.buffer.write(b"mock-image")
    sys.exit(0)
service, operation = args[:2]
if "--generate-cli-skeleton" in args:
    emit({"arn": ""})
elif (service, operation) == ("sts", "get-caller-identity"):
    if "--query" in args: print("123456789012")
    else: emit({"Account": "123456789012"})
elif (service, operation) == ("freetier", "get-account-plan-state"):
    emit({"accountPlanType": "FREE", "accountPlanStatus": os.environ.get("FAKE_PLAN_STATUS", "ACTIVE"), "accountPlanRemainingCredits": {"amount": float(os.environ.get("FAKE_CREDITS", "100"))}, "accountPlanExpirationDate": "2027-05-01T00:00:00Z"})
elif (service, operation) == ("ec2", "describe-instance-type-offerings"):
    emit({"InstanceTypeOfferings": [] if os.environ.get("FAKE_NO_OFFERING") else [{"InstanceType": "t4g.small", "Location": "us-east-1b"}]})
elif (service, operation) == ("ec2", "describe-availability-zones"):
    emit({"AvailabilityZones": [{"ZoneName": "us-east-1a"}, {"ZoneName": "us-east-1b"}]})
elif (service, operation) == ("ec2", "describe-managed-prefix-lists"):
    print("pl-0123456789abcdef0")
elif (service, operation) == ("ec2", "describe-instances"):
    print("ec2-203-0-113-1.compute-1.amazonaws.com")
elif (service, operation) == ("ec2", "describe-volumes"):
    print(os.environ.get("FAKE_VOLUME_STATE", "available"))
elif (service, operation) == ("ssm", "get-parameter"):
    if arg("--name").startswith("/aws/service/"): print("ami-0123456789abcdef0")
    elif (root / "token.json").exists(): emit({"Parameter": {"Value": json.loads((root / "token.json").read_text())["Value"]}})
    else: fail("ParameterNotFound")
elif (service, operation) == ("ssm", "put-parameter"):
    (root / "token.json").write_text(Path(arg("--cli-input-json")[7:]).read_text())
    emit({"Version": 1})
elif (service, operation) == ("cloudformation", "describe-stacks"):
    if (root / "created").exists(): emit(stack())
    else: fail("ValidationError: Stack with id poketeam does not exist")
elif (service, operation) == ("cloudformation", "validate-template"):
    emit({"Parameters": []})
elif (service, operation) == ("cloudformation", "deploy"):
    (root / "created").touch()
    (root / "template.json").write_text(Path(arg("--template-file")).read_text())
    emit({})
elif (service, operation) == ("cloudformation", "get-template"):
    emit({"TemplateBody": json.loads((root / "template.json").read_text())})
elif (service, operation) == ("pricing-plan-manager", "get-subscription"):
    emit({"subscription": {"status": "ACTIVE", "planTier": os.environ.get("FAKE_CF_TIER", "FREE")}})
elif (service, operation) == ("ssm", "describe-instance-information"):
    print("Online")
elif (service, operation) == ("ssm", "send-command"):
    (root / "remote.json").write_text(Path(arg("--parameters")[7:]).read_text())
    print("command-123")
elif (service, operation) == ("ssm", "get-command-invocation"):
    emit({"Status": "Success", "StandardOutputContent": "Mock deployment completed.", "StandardErrorContent": ""})
elif (service, operation) == ("cloudformation", "list-stack-resources"):
    emit({"StackResourceSummaries": [{"LogicalResourceId": "DataVolume", "PhysicalResourceId": "vol-0123456789abcdef0"}, {"LogicalResourceId": "Bucket", "PhysicalResourceId": "mock-private-bucket"}]})
elif (service, operation) in (("cloudformation", "delete-stack"), ("cloudformation", "wait"), ("s3", "cp"), ("s3", "rm"), ("s3api", "head-bucket"), ("s3api", "delete-bucket"), ("ec2", "delete-volume"), ("ssm", "delete-parameter")):
    emit({})
else:
    fail(f"Unexpected offline command: {args}")
