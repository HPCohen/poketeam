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


def payload():
    return json.loads(Path(arg("--cli-input-json")[7:]).read_text())


def subscription():
    value = json.loads((root / "subscription.json").read_text())
    value["planTier"] = os.environ.get("FAKE_CF_TIER", "FREE")
    if (root / "cancelled").exists(): value["scheduledChange"] = {"changeType": "CANCELLATION"}
    return value


def stack():
    values = {
        "InstanceId": "i-0123456789abcdef0", "BucketName": "mock-private-bucket", "DataVolumeId": "vol-0123456789abcdef0",
        "OriginDomainName": "ec2-203-0-113-1.compute-1.amazonaws.com",
    }
    if os.environ.get('AWS_REGION') == 'us-east-1':
        values.update(DistributionId="DIST123", SubscriptionStatus="ACTIVE", SubscriptionArn="arn:aws:pricingplanmanager::123456789012:subscription:example", PublicOrigin="https://example.cloudfront.net")
    return {"Stacks": [{"StackStatus": "CREATE_COMPLETE", "Outputs": [
        {"OutputKey": k, "OutputValue": v} for k,v in values.items()]}]}


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
    emit({"InstanceTypeOfferings": [] if os.environ.get("FAKE_NO_OFFERING") else [{"InstanceType": "t4g.small", "Location": os.environ.get("AWS_REGION", "us-east-1") + "b"}]})
elif (service, operation) == ("ec2", "describe-availability-zones"):
    emit({"AvailabilityZones": [{"ZoneName": os.environ.get("AWS_REGION", "us-east-1") + "a"}, {"ZoneName": os.environ.get("AWS_REGION", "us-east-1") + "b"}]})
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
    emit({"subscription": subscription(), "eTag": "subscription-etag"} if (root / "subscription.json").exists() else {"subscription": {"status": "ACTIVE", "planTier": os.environ.get("FAKE_CF_TIER", "FREE")}})
elif (service, operation) == ("pricing-plan-manager", "list-subscriptions"):
    if os.environ.get("FAKE_SUBSCRIPTION_DENIED"): fail("AccessDeniedException: explicit deny in service control policy")
    emit({"subscriptionSummaries": [subscription()] if (root / "subscription.json").exists() and not os.environ.get("FAKE_SUBSCRIPTION_EXPIRED") else []})
elif (service, operation) == ("pricing-plan-manager", "create-subscription"):
    value = dict(payload(), arn="arn:aws:pricingplanmanager::123456789012:subscription:edge", status="ACTIVE")
    (root / "subscription.json").write_text(json.dumps(value))
    emit({"subscription": value})
elif (service, operation) == ("pricing-plan-manager", "cancel-subscription"):
    assert payload()['ifMatch'] == 'subscription-etag'
    (root / "cancelled").touch()
    emit({"subscription": subscription()})
elif (service, operation) == ("wafv2", "list-web-acls"):
    emit({"WebACLs": [json.loads((root / "acl.json").read_text())] if (root / "acl.json").exists() else []})
elif (service, operation) == ("wafv2", "create-web-acl"):
    config = payload()
    value = dict(config, Id="acl-123", ARN="arn:aws:wafv2:us-east-1:123456789012:global/webacl/test/acl-123")
    (root / "acl.json").write_text(json.dumps(value))
    emit({"Summary": value})
elif (service, operation) == ("wafv2", "list-tags-for-resource"):
    emit({"TagInfoForResource": {"TagList": json.loads((root / "acl.json").read_text())["Tags"]}})
elif (service, operation) == ("wafv2", "get-web-acl"):
    emit({"WebACL": json.loads((root / "acl.json").read_text()), "LockToken": "acl-lock"})
elif (service, operation) == ("wafv2", "delete-web-acl"):
    assert payload()['LockToken'] == 'acl-lock'
    (root / "acl.json").unlink()
    emit({})
elif (service, operation) == ("cloudfront", "list-distributions"):
    config = json.loads((root / "distribution.json").read_text()) if (root / "distribution.json").exists() else None
    emit({"DistributionList": {"Items": [{"Id": "DIST-EDGE", "Comment": config["Comment"]}] if config else []}})
elif (service, operation) == ("cloudfront", "create-distribution-with-tags"):
    config = payload()['DistributionConfigWithTags']['DistributionConfig']
    (root / "distribution.json").write_text(json.dumps(config))
    emit({"Distribution": {"Id": "DIST-EDGE", "ARN": "arn:aws:cloudfront::123456789012:distribution/DIST-EDGE", "DomainName": "edge.cloudfront.net", "DistributionConfig": config}})
elif (service, operation) == ("cloudfront", "get-distribution"):
    config = json.loads((root / "distribution.json").read_text())
    emit({"ETag": "distribution-etag", "Distribution": {"Id": "DIST-EDGE", "ARN": "arn:aws:cloudfront::123456789012:distribution/DIST-EDGE", "DomainName": "edge.cloudfront.net", "DistributionConfig": config}})
elif (service, operation) == ("cloudfront", "update-distribution"):
    request = payload()
    assert request['IfMatch'] == 'distribution-etag'
    (root / "distribution.json").write_text(json.dumps(request['DistributionConfig']))
    emit({})
elif (service, operation) == ("cloudfront", "delete-distribution"):
    assert payload()['IfMatch'] == 'distribution-etag'
    assert not json.loads((root / "distribution.json").read_text())['Enabled']
    (root / "distribution.json").unlink()
    emit({})
elif (service, operation) == ("cloudfront", "wait"):
    emit({})
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
