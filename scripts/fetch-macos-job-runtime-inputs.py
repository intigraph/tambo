#!/usr/bin/env python3
"""Unprivileged CI input fetch: fixed reviewed HTTPS inputs, verified before use."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import urllib.request

ROOT = Path(__file__).resolve().parents[1]


def fetch(directory):
    if os.geteuid() == 0:
        raise ValueError('never download runtime inputs as root')
    pins = json.loads((ROOT / 'docs/operator-packets/e579/runtime/sources.json').read_text())
    directory.mkdir(mode=0o700)
    for name in ('python', 'shantytown', 'seeds'):
        row = pins[name]
        if not row['url'].startswith(('https://github.com/', 'https://codeload.github.com/')):
            raise ValueError('reviewed HTTPS public source required')
        digest = hashlib.sha256()
        count = 0
        path = directory / (name + '.tar.gz')
        with urllib.request.urlopen(row['url'], timeout=90) as response, path.open('xb') as output:
            if not response.url.startswith('https://'):
                raise ValueError('refuse downgraded download')
            while True:
                chunk = response.read(1048576)
                if not chunk:
                    break
                count += len(chunk)
                if count > 256 * 1024 * 1024:
                    raise ValueError('archive exceeds bounded download')
                digest.update(chunk)
                output.write(chunk)
        if digest.hexdigest() != row['sha256']:
            raise ValueError('runtime source digest mismatch; retain input, do not build')
        print(name + ': verified reviewed archive')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--directory', type=Path, required=True)
    fetch(parser.parse_args().directory)
