#!/usr/bin/env python3
"""Render a private reviewed identity candidate; never enroll keys or install policy."""
import argparse
import hashlib
import json
from pathlib import Path
import re
from urllib.parse import urlsplit


def render(principal, endpoint):
    parsed = urlsplit(endpoint)
    if (not re.fullmatch('[A-Za-z0-9][A-Za-z0-9_.:-]{0,127}', principal)
            or principal == 'eph-task-000001' or parsed.scheme != 'https' or not parsed.hostname
            or parsed.username or parsed.password or parsed.query or parsed.fragment
            or any(value in endpoint for value in ('\n', '\r', '\0'))):
        raise ValueError('explicit distinct controller principal and credential-free HTTPS endpoint required')
    return (json.dumps({'version': 1, 'required': True, 'users': {
        '51000': {'principal': principal, 'home': '/var/lib/shantytown/controllers/_stctl',
                  'endpoint': endpoint, 'sd': '/opt/shantytown-job-launcher/bin/sd'},
        '51001': {'principal': 'eph-task-000001', 'home': '/var/lib/shantytown/agents/crew-eph-task-000001',
                  'endpoint': endpoint, 'sd': '/opt/shantytown-job-launcher/bin/sd'}}}, indent=2, sort_keys=True) + '\n').encode()


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--controller-principal', required=True)
    parser.add_argument('--endpoint', required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    raw = render(args.controller_principal, args.endpoint)
    with args.output.open('xb') as stream:
        args.output.chmod(0o600)
        stream.write(raw)
    print('candidate_sha256=' + hashlib.sha256(raw).hexdigest() + '; NOT INSTALLED; key proofs required')
