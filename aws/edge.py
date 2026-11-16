#!/usr/bin/env python3
"""Project-compatible global edge lifecycle. Credentials and signing stay in AWS CLI."""
import json
import os
from pathlib import Path
import subprocess
import sys


def read(path):
    return json.loads(path.read_text())


def write(path, value):
    temporary = path.with_suffix('.tmp')
    temporary.write_text(json.dumps(value, indent=2) + '\n')
    temporary.replace(path)


class Edge:
    def __init__(self, directory):
        self.directory = Path(directory)
        self.path = self.directory / 'edge.json'
        self.state = read(self.path) if self.path.exists() else {}
        self.account = read(self.directory / 'identity.json')['Account']
        self.region = os.environ['AWS_REGION']
        self.stack = os.environ['STACK_NAME']
        if self.region != 'us-east-2':
            raise ValueError('Direct edge lifecycle is only used with us-east-2 regional stacks.')
        self.name = f'{self.stack}-{self.account}-{self.region}'
        if 'subscription_arn' in self.state or 'web_acl_id' in self.state:
            raise ValueError('Existing subscription/WAF edge state requires review before switching to standard pricing.')
        self.tags = [{'Key': 'Project', 'Value': self.stack}, {'Key': 'ProjectRegion', 'Value': self.region}]

    def save(self):
        write(self.path, self.state)

    def aws(self, service, operation, payload=None, *args):
        command = ['aws', service, operation, '--region', 'us-east-1', '--output', 'json']
        if payload is not None:
            request = self.directory / 'edge-request.json'
            write(request, payload)
            command += ['--cli-input-json', f'file://{request}']
        result = subprocess.run(command + list(args), capture_output=True, text=True)
        if result.returncode:
            # Do not include request JSON: distribution headers contain the origin secret.
            raise RuntimeError(f'{service} {operation} failed:\n{result.stderr.strip()}')
        return json.loads(result.stdout) if result.stdout.strip() else {}

    def distribution(self):
        result = self.aws('cloudfront', 'get-distribution', {'Id': self.state['distribution_id']})
        distribution = result['Distribution']
        if distribution['DistributionConfig']['CallerReference'] != self.name:
            raise ValueError('Saved distribution does not belong to this account/region/stack lifecycle.')
        return result

    def check(self):
        if self.state.get('pricing') != 'standard':
            raise ValueError('Edge must be explicitly created with standard usage-based pricing.')
        result = self.distribution()
        if result['Distribution']['DistributionConfig'].get('WebACLId'):
            raise ValueError('Standard deployment must not have an attached WAF with separate charges.')
        return result

    def create(self):
        if 'distribution_id' not in self.state:
            items = self.aws('cloudfront', 'list-distributions').get('DistributionList', {}).get('Items', [])
            matches = [item for item in items if item.get('Comment') == self.name]
            if len(matches) > 1:
                raise ValueError('Multiple matching edge distributions; inspect before continuing.')
            if matches:
                distribution = self.aws('cloudfront', 'get-distribution', {'Id': matches[0]['Id']})['Distribution']
                if distribution['DistributionConfig']['CallerReference'] != self.name:
                    raise ValueError('Refusing to adopt a distribution with a different caller reference.')
            else:
                config = read(Path(__file__).with_name('stack.template.json'))['Resources']['Distribution']['Properties']['DistributionConfig']
                config.update(CallerReference=self.name, Comment=self.name, Enabled=False, WebACLId='', PriceClass='PriceClass_100')
                outputs = {o['OutputKey']: o['OutputValue'] for o in read(self.directory / 'stack.json')['Stacks'][0]['Outputs']}
                token = read(self.directory / 'token.json')['Parameter']['Value']
                config['Origins'][0]['DomainName'] = outputs['OriginDomainName']
                config['Origins'][0]['OriginCustomHeaders'][0]['HeaderValue'] = token
                # CLI uses API shapes (Quantity/Items), unlike CloudFormation's arrays.
                origins = config.pop('Origins')
                for origin in origins:
                    headers = origin.pop('OriginCustomHeaders')
                    origin['CustomHeaders'] = {'Quantity': len(headers), 'Items': headers}
                    origin['CustomOriginConfig']['OriginSslProtocols'] = {'Quantity': 1, 'Items': ['TLSv1.2']}
                config['Origins'] = {'Quantity': len(origins), 'Items': origins}
                behavior = config['DefaultCacheBehavior']
                methods = behavior['AllowedMethods']
                cached = behavior.pop('CachedMethods')
                behavior['AllowedMethods'] = {'Quantity': len(methods), 'Items': methods, 'CachedMethods': {'Quantity': len(cached), 'Items': cached}}
                errors = config['CustomErrorResponses']
                config['CustomErrorResponses'] = {'Quantity': len(errors), 'Items': errors}
                # API spelling differs from CloudFormation's IPV6Enabled.
                config['IsIPV6Enabled'] = config.pop('IPV6Enabled')
                distribution = self.aws('cloudfront', 'create-distribution-with-tags', {
                    'DistributionConfigWithTags': {'DistributionConfig': config, 'Tags': {'Items': self.tags}}
                })['Distribution']
            self.state.update(pricing='standard', distribution_id=distribution['Id'], distribution_arn=distribution['ARN'], domain=distribution['DomainName'])
            self.save()
        self.check()
        self.outputs()
        print('Global edge created/resumed. New distributions remain disabled until backend deployment.')

    def enable(self):
        self.check()
        result = self.distribution()
        config = result['Distribution']['DistributionConfig']
        config['Enabled'] = True
        config['Origins']['Items'][0]['DomainName'] = (self.directory / 'origin-dns').read_text().strip()
        self.aws('cloudfront', 'update-distribution', {
            'Id': self.state['distribution_id'], 'IfMatch': result['ETag'], 'DistributionConfig': config,
        })
        self.wait()

    def wait(self):
        self.aws('cloudfront', 'wait', None, 'distribution-deployed', '--id', self.state['distribution_id'])

    def destroy(self):
        # Caller checks CONFIRM_DESTROY; deletion never runs as part of create/deploy.
        if 'distribution_id' in self.state:
            result = self.distribution()
            if result['Distribution']['DistributionConfig']['Enabled']:
                config = result['Distribution']['DistributionConfig']
                config['Enabled'] = False
                self.aws('cloudfront', 'update-distribution', {
                    'Id': self.state['distribution_id'], 'IfMatch': result['ETag'], 'DistributionConfig': config,
                })
            self.wait()
        if 'distribution_id' in self.state:
            result = self.distribution()
            self.aws('cloudfront', 'delete-distribution', {'Id': self.state['distribution_id'], 'IfMatch': result['ETag']})
            for key in ('distribution_id', 'distribution_arn', 'domain'):
                del self.state[key]
            self.save()
        print('Global edge resources deleted.')

    def outputs(self):
        path = self.directory / 'stack.json'
        stack = read(path)
        values = {'DistributionId': self.state['distribution_id'], 'PublicOrigin': 'https://' + self.state['domain']}
        outputs = stack['Stacks'][0]['Outputs']
        outputs[:] = [o for o in outputs if o['OutputKey'] not in values]
        outputs.extend({'OutputKey': key, 'OutputValue': value} for key, value in values.items())
        write(path, stack)


if __name__ == '__main__':
    os.umask(0o077)
    try:
        action, directory = sys.argv[1:]
        edge = Edge(directory)
        if action not in ('create', 'check', 'enable', 'outputs', 'destroy'):
            raise ValueError('Unknown edge operation')
        getattr(edge, action)()
    except (ValueError, KeyError, RuntimeError) as error:
        sys.exit(str(error))
