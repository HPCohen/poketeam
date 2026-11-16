import contextlib
import datetime as dt
import importlib.util
import io
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest
from unittest.mock import patch

AWS = Path(__file__).resolve().parents[1]


def load(name):
    spec = importlib.util.spec_from_file_location(name, AWS / (name + ".py"))
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


class InfrastructureTests(unittest.TestCase):
    def test_cost_and_data_invariants(self):
        template = load("template").template("us-east-1")
        resources = template["Resources"]
        self.assertEqual(sum(r["Type"] == "AWS::EC2::Instance" for r in resources.values()), 1)
        self.assertFalse(any(any(x in r["Type"] for x in ("NATGateway", "DBInstance", "LoadBalancer", "ECR::Repository")) for r in resources.values()))
        for name in ("Bucket", "DataVolume"):
            self.assertEqual(resources[name]["DeletionPolicy"], "Retain")
        host = resources["Host"]["Properties"]
        self.assertEqual(host["CreditSpecification"]["CPUCredits"], "standard")
        self.assertEqual(host["MetadataOptions"]["HttpTokens"], "required")
        ingress = resources["SecurityGroup"]["Properties"]["SecurityGroupIngress"]
        self.assertEqual([r["FromPort"] for r in ingress], [80])
        self.assertNotIn("CidrIp", ingress[0])
        self.assertEqual(resources["Subscription"]["Properties"]["PlanTier"], "FREE")
        distribution = resources["Distribution"]["Properties"]["DistributionConfig"]
        self.assertEqual(distribution["Enabled"], {"Fn::If": ["TrafficEnabled", True, False]})
        self.assertEqual(distribution["DefaultCacheBehavior"]["CachePolicyId"], "4135ea2d-6df8-44a3-9df3-4b5a84be39ad")
        self.assertLess(len(json.dumps(template).encode()), 51200)

    def test_regional_template_has_no_global_edge_or_secret(self):
        template = load("template").template("us-east-2")
        self.assertFalse(any(r['Type'].startswith(('AWS::WAF', 'AWS::CloudFront', 'AWS::PricingPlan')) for r in template['Resources'].values()))
        self.assertNotIn('OriginToken', template['Parameters'])
        self.assertNotIn('Conditions', template)
        self.assertIn('OriginDomainName', template['Outputs'])

    def test_trial_expiry_changes_new_instance_default(self):
        manage = load("manage")
        with patch.dict(os.environ, {"INSTANCE_TYPE": ""}):
            self.assertEqual(manage.instance_type(dt.date(2026,12,31)), "t4g.small")
            self.assertEqual(manage.instance_type(dt.date(2027,1,1)), "t4g.micro")


class CliTestCase(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name)
        self.aws = self.root / "aws"
        shutil.copytree(AWS, self.aws, ignore=shutil.ignore_patterns(".state", "config.sh", "__pycache__"))
        self.bin = self.root / "bin"
        self.bin.mkdir()
        for name in ("aws", "docker"):
            (self.bin / name).symlink_to(self.aws / "tests/fake_aws.py")
        self.fake = self.root / "fake"
        self.env = dict(os.environ, PATH=str(self.bin) + os.pathsep + os.environ["PATH"], FAKE_STATE=str(self.fake), STACK_NAME="poketeam", AWS_REGION="us-east-1", BUDGET_EMAIL="owner@example.com", ALLOW_PAID_RESOURCES="0", INSTANCE_TYPE="t4g.small")
        self.env.pop("AWS_DEFAULT_REGION", None)

    def tearDown(self):
        self.temp.cleanup()

    def run_script(self, name, *args, expected=0, **env):
        result = subprocess.run(["bash", str(self.aws / name), *args], env=dict(self.env, **env), text=True, capture_output=True)
        self.assertEqual(result.returncode, expected, result.stdout + result.stderr)
        return result

    def calls(self):
        path = self.fake / "calls.jsonl"
        return [json.loads(s) for s in path.read_text().splitlines()] if path.exists() else []


class CliLifecycleTests(CliTestCase):
    def test_expired_coverage_fails_before_mutation(self):
        self.run_script("create.sh", expected=1, FAKE_PLAN_STATUS="EXPIRED")
        self.assertFalse(any(c[1:3] in (["cloudformation","deploy"], ["ssm","put-parameter"]) for c in self.calls()))

    def test_no_credits_fails_before_mutation(self):
        self.run_script("create.sh", expected=1, FAKE_CREDITS="0")
        self.assertFalse((self.fake / "created").exists())

    def test_unavailable_instance_fails(self):
        self.run_script("create.sh", expected=1, FAKE_NO_OFFERING="1")
        self.assertFalse((self.fake / "created").exists())

    def test_create_is_repeatable_and_preserves_token(self):
        self.run_script("create.sh")
        state = self.aws / ".state/123456789012/us-east-1/poketeam"
        parameters = dict(item.split("=",1) for item in json.loads((state / "parameters.json").read_text()))
        self.assertEqual(parameters["EnableTraffic"], "false")
        self.assertEqual(parameters["AvailabilityZone"], "us-east-1b")
        self.assertEqual((state / "parameters.json").stat().st_mode & 0o777, 0o600)
        token = (self.fake / "token.json").read_text()
        self.run_script("create.sh")
        self.assertEqual(token, (self.fake / "token.json").read_text())
        self.assertEqual(sum(c[1:3] == ["cloudformation","deploy"] for c in self.calls()), 1)

    def test_backend_deploy_preserves_noecho_and_builds_arm(self):
        self.run_script("create.sh")
        self.run_script("deploy-backend.sh")
        calls = self.calls()
        self.assertTrue(any(c[0] == "docker" and "linux/arm64" in c for c in calls))
        state = self.aws / ".state/123456789012/us-east-1/poketeam"
        self.assertEqual(json.loads((state / "enable-parameters.json").read_text()), ["EnableTraffic=true", "OriginDomainName=ec2-203-0-113-1.compute-1.amazonaws.com"])
        command = json.loads((self.fake / "remote.json").read_text())["commands"][0]
        token = json.loads((self.fake / "token.json").read_text())["Value"]
        self.assertNotIn(token, command)
        self.assertIn("cloud-init status --wait", command)
        subprocess.run(["bash", "-n"], input=command, text=True, check=True)

    def test_paid_cloudfront_tier_blocks_deployment(self):
        self.run_script("create.sh")
        self.run_script("deploy-backend.sh", expected=1, FAKE_CF_TIER="PRO")
        self.assertFalse(any(c[0] == "docker" for c in self.calls()))

    def test_frontend_rejects_symlinks(self):
        self.run_script("create.sh")
        build = self.root / "dist"
        build.mkdir()
        (build / "index.html").write_text("frontend")
        (build / "private-link").symlink_to(self.root)
        self.run_script("deploy-frontend.sh", str(build), expected=1)
        self.assertFalse(any(c[1:3] == ["s3", "cp"] for c in self.calls()))

    def test_frontend_and_data_teardown_guards(self):
        self.run_script("create.sh")
        build = self.root / "dist"
        build.mkdir()
        (build / "index.html").write_text("frontend")
        self.run_script("deploy-frontend.sh", str(build))
        command = json.loads((self.fake / "remote.json").read_text())["commands"][0]
        self.assertIn("/srv/poketeam/public/current", command)
        subprocess.run(["bash", "-n"], input=command, text=True, check=True)
        self.run_script("destroy.sh", expected=1)
        self.run_script("destroy.sh", CONFIRM_DESTROY="poketeam")
        self.assertFalse(any(c[1:3] == ["ec2", "delete-volume"] for c in self.calls()))
        self.run_script("purge-data.sh", expected=1)
        self.run_script("purge-data.sh", expected=1, CONFIRM_PURGE="poketeam:123456789012", FAKE_VOLUME_STATE="in-use")
        self.assertFalse(any(c[1:3] == ["ec2", "delete-volume"] for c in self.calls()))
        self.run_script("purge-data.sh", CONFIRM_PURGE="poketeam:123456789012")
        self.assertTrue(any(c[1:3] == ["ec2", "delete-volume"] for c in self.calls()))


class ProjectLifecycleTests(CliTestCase):
    def setUp(self):
        super().setUp()
        self.env['AWS_REGION'] = 'us-east-2'

    def test_create_is_repeatable_and_preserves_token(self):
        self.run_script('create.sh')
        state = self.aws / '.state/123456789012/us-east-2/poketeam'
        config = json.loads((self.fake / 'distribution.json').read_text())
        self.assertFalse(config['Enabled'])
        self.assertTrue(config['IsIPV6Enabled'])
        self.assertEqual(config['DefaultCacheBehavior']['AllowedMethods']['CachedMethods']['Items'], ['GET', 'HEAD'])
        self.assertEqual(config['Origins']['Items'][0]['CustomHeaders']['Quantity'], 1)
        self.assertNotIn('OriginToken=', (state / 'parameters.json').read_text())
        token = (self.fake / 'token.json').read_text()
        # Recover even when the last response was lost before private IDs were saved.
        (state / 'edge.json').unlink()
        self.run_script('create.sh')
        self.assertEqual(token, (self.fake / 'token.json').read_text())
        for command in (['cloudformation', 'deploy'], ['cloudfront', 'create-distribution-with-tags']):
            self.assertEqual(sum(c[1:3] == command for c in self.calls()), 1)
        self.assertFalse(any(c[1] == 'cloudformation' and 'us-east-1' in c for c in self.calls()))
        self.assertTrue(all(c[c.index('--region') + 1] == 'us-east-1' for c in self.calls() if c[1] in ('wafv2', 'cloudfront') and '--generate-cli-skeleton' not in c))

    def test_backend_deploy_preserves_noecho_and_builds_arm(self):
        self.run_script('create.sh')
        self.run_script('deploy-backend.sh')
        config = json.loads((self.fake / 'distribution.json').read_text())
        self.assertTrue(config['Enabled'])
        self.assertEqual(config['Origins']['Items'][0]['DomainName'], 'ec2-203-0-113-1.compute-1.amazonaws.com')
        self.assertTrue(any(c[0] == 'docker' and 'linux/arm64' in c for c in self.calls()))
        token = json.loads((self.fake / 'token.json').read_text())['Value']
        self.assertNotIn(token, json.dumps(self.calls()))
        self.assertNotIn(token, (self.fake / 'remote.json').read_text())
        self.assertEqual(sum(c[1:3] == ['cloudformation', 'deploy'] for c in self.calls()), 1)

    def test_standard_pricing_never_calls_subscription_or_waf_apis(self):
        self.run_script('create.sh', FAKE_SUBSCRIPTION_DENIED='1')
        self.run_script('deploy-backend.sh', FAKE_SUBSCRIPTION_DENIED='1')
        self.assertFalse(any(c[1] in ('wafv2', 'pricing-plan-manager') for c in self.calls()))
        self.assertEqual(json.loads((self.fake / 'distribution.json').read_text())['WebACLId'], '')

    def test_existing_subscription_state_requires_explicit_review(self):
        self.run_script('create.sh')
        state = self.aws / '.state/123456789012/us-east-2/poketeam/edge.json'
        value = json.loads(state.read_text())
        value['subscription_arn'] = 'existing-subscription'
        state.write_text(json.dumps(value))
        self.run_script('deploy-backend.sh', expected=1)
        self.assertFalse(any(c[0] == 'docker' for c in self.calls()))

    def test_attached_waf_blocks_enablement(self):
        self.run_script('create.sh')
        path = self.fake / 'distribution.json'
        value = json.loads(path.read_text())
        value['WebACLId'] = 'unexpected-waf'
        path.write_text(json.dumps(value))
        self.run_script('deploy-backend.sh', expected=1)
        self.assertFalse(json.loads(path.read_text())['Enabled'])

    def test_frontend_and_data_teardown_guards(self):
        self.run_script('create.sh')
        self.run_script('deploy-backend.sh')
        self.run_script('destroy.sh', expected=1)
        self.run_script('destroy.sh', CONFIRM_DESTROY='poketeam')
        self.assertFalse((self.fake / 'distribution.json').exists())
        self.assertFalse(any(c[1] in ('wafv2', 'pricing-plan-manager') for c in self.calls()))
        self.assertFalse(any(c[1:3] == ['ec2', 'delete-volume'] for c in self.calls()))


if __name__ == "__main__":
    unittest.main()
