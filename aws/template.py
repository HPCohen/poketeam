#!/usr/bin/env python3
"""Embed the host bootstrap in the otherwise static, reviewable CloudFormation template."""
import json
from pathlib import Path


def template():
    directory = Path(__file__).parent
    value = json.loads((directory / "stack.template.json").read_text())
    parts = value["Resources"]["Host"]["Properties"]["UserData"]["Fn::Base64"]["Fn::Join"][1]
    assert parts[-1] == "__HOST_BOOTSTRAP__"
    parts[-1] = (directory / "host/bootstrap.sh").read_text()
    return value


if __name__ == "__main__":
    print(json.dumps(template(), indent=2))
