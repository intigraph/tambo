#!/usr/bin/env python3
"""Read-only source contract demonstration; no native account or launch."""
import importlib.util
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
loader = importlib.util.spec_from_file_location('migration', ROOT / 'scripts/macos-job-manifest-migration.py')
m = importlib.util.module_from_spec(loader)
loader.loader.exec_module(m)
old = (ROOT / 'docs/operator-packets/e579/catalog/eph-task-000001.json').read_bytes()
base = json.loads(old)
print('Target: retained provisioning-only fixture -> candidate source, no native execution')
print('BASE manifest SHA256:', m.packet.digest(old))
print('BASE missing launch fields:', len((m.IDENTITY | m.LAUNCH) - set(base)))
home = base['home']
fields = {'controller_root': '/private/var/lib/shantytown/controllers/_stctl/root',
          'workspace': home + '/workspace', 'settings': home + '/settings.json',
          'harness': 'claude', 'request': {'job': 'task', 'item': 'fixture-item', 'repo': 'example/fixture',
          'pr': None, 'checkout': None, 'head': None, 'request_id': None}, 'review': None,
          'argv': ['/opt/shantytown-job-launcher/bin/python3', '--version'],
          'environment': {'PATH': '/usr/bin:/bin'}}
raw = m.candidate(old, m.encoded(fields))
row = json.loads(raw)
print('CANDIDATE missing launch fields:', len((m.IDENTITY | m.LAUNCH) - set(row)))
print('Original eight identity values retained:', {k: row[k] for k in m.IDENTITY} == base)
print('Original eight identity serialized bytes retained:', raw.startswith(old[:old.rfind(b'}')].rstrip() + b','))
try:
    m.candidate(old, m.encoded(dict(fields, uid=51002)))
except ValueError as error:
    print('NEGATIVE identity edit: REFUSED:', error)
else:
    raise AssertionError('identity edit admitted')
print('Native accounts, CA, keys, runtime apply and lifecycle: UNPROVED; readiness=false')
