#!/usr/bin/env python3
"""Show the effective runtime input revision; never execute Mac code on Linux."""
import argparse
import json
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[3]
BASE = '7fd277747df3ea63a50c485c7b1f566654eb6b0d'
PACKET = 'docs/operator-packets/e579/runtime/'


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--directory', type=Path, required=True)
    args = parser.parse_args()
    before = json.loads(subprocess.check_output(
        ['git', '-C', str(ROOT), 'show', BASE + ':' + PACKET + 'sources.json']))
    old = json.loads(subprocess.check_output(
        ['git', '-C', str(ROOT), 'show', BASE + ':' + PACKET + 'artifact.json']))
    after = json.loads((ROOT / PACKET / 'sources.json').read_text())
    artifact = json.loads((ROOT / PACKET / 'artifact.json').read_text())
    for label, sources, runtime in [('before', before, old), ('after', after, artifact)]:
        print(label + ': Seeds ' + sources['seeds']['version'] + '; ST ' + sources['shantytown']['commit'], flush=True)
        print(label + ': manifest ' + runtime['manifest_sha256'], flush=True)
    assert before['seeds']['version'] == '0.13.0'
    assert after['seeds']['version'] == '0.14.0'
    assert artifact['manifest_sha256'] != old['manifest_sha256']
    assert artifact['native_acceptance'] is False
    subprocess.run([sys.executable, str(ROOT / 'scripts/verify-macos-job-runtime.py'),
                    '--directory', str(args.directory)], check=True)
    print('Public input and exact inventory proof only; native UID/CA acceptance remains OPEN.', flush=True)


if __name__ == '__main__':
    main()
