#!/usr/bin/env python3
"""Embed the host bootstrap in the otherwise static, reviewable CloudFormation template."""
import json
import os
from pathlib import Path


def template(region=None):
    directory = Path(__file__).parent
    value = json.loads((directory / "stack.template.json").read_text())
    parts = value["Resources"]["Host"]["Properties"]["UserData"]["Fn::Base64"]["Fn::Join"][1]
    assert parts[-1] == "__HOST_BOOTSTRAP__"
    parts[-1] = (directory / "host/bootstrap.sh").read_text()
    if (region or os.environ.get("AWS_REGION", "us-east-1")) == "us-east-2":
        value["Description"] = "Poketeam regional host; global edge managed separately through service APIs."
        for key in ("OriginToken", "EnableTraffic", "OriginDomainName"):
            del value["Parameters"][key]
        del value["Conditions"]
        for key in ("WebAcl", "Distribution", "Subscription"):
            del value["Resources"][key]
        for key in ("DistributionId", "SubscriptionStatus", "SubscriptionArn", "PublicOrigin"):
            del value["Outputs"][key]
    return value


if __name__ == "__main__":
    print(json.dumps(template(), indent=2))
