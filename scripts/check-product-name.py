#!/usr/bin/env python3
"""Keep reviewed legacy contracts exact while rejecting new old-brand references."""
# arming: ci just check
import argparse
from collections import Counter
import hashlib
import json
from pathlib import Path
import re
import subprocess

PATTERN = re.compile('caboodle', re.IGNORECASE)
LEDGER = 'docs/compatibility/legacy-name-contracts.json'


def inventory(root):
    result = {}
    names = subprocess.check_output(
        ['git', 'ls-files', '-z', '--cached', '--others', '--exclude-standard'], cwd=root
    ).decode().split('\0')
    for name in sorted(set(filter(None, names)) - {LEDGER}):
        path = root / name
        if not path.is_file() or path.is_symlink():
            continue
        try:
            text = path.read_text(encoding='utf-8')
        except UnicodeDecodeError:
            continue
        matches = Counter(
            hashlib.sha256(line.encode()).hexdigest()
            for line in text.splitlines() if PATTERN.search(line)
        )
        if matches:
            result[name] = dict(sorted(matches.items()))
    return result


def check(root, ledger):
    document = json.loads(ledger.read_text())
    if document.get('version') != 1:
        raise ValueError('unsupported compatibility inventory version')
    contracts = document['files']
    if any(not entry.get('reason', '').strip() for entry in contracts.values()):
        raise ValueError('every retained file needs a compatibility reason')
    actual = inventory(root)
    expected = {path: entry['lines'] for path, entry in contracts.items()}
    changed = sorted(path for path in actual.keys() | expected.keys()
                     if actual.get(path) != expected.get(path))
    return changed, sum(sum(lines.values()) for lines in actual.values())


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--root', type=Path, default=Path(__file__).resolve().parents[1])
    args = parser.parse_args()
    changed, count = check(args.root, args.root / LEDGER)
    if changed:
        print('Unreviewed legacy-name inventory changes:')
        print('\n'.join(changed))
        return 1
    print(f'Reviewed compatibility inventory: {count} references; no unclassified changes')
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
