#!/usr/bin/env python3
# /// script
# requires-python = ">=3.11"
# dependencies = []
# ///
"""Replay the default bootstrap on immutable scripts and owned release fixtures."""
import pathlib
import runpy
import subprocess
import tempfile

REPO = pathlib.Path(__file__).resolve().parents[3]
BEFORE = 'a696fccd13157db1943bbb4c99ed76d8cf84a93f'
AFTER = '47d3ee0bf6ececc0150968295fee7d1a221a7baa'
fixture = runpy.run_path(str(REPO / 'scripts/test-install-bootstrap.py'))
Bootstrap = fixture['Bootstrap']
for label, revision, expected in [('BEFORE', BEFORE, 1), ('AFTER', AFTER, 0)]:
    with tempfile.TemporaryDirectory(prefix='bootstrap-source-') as directory:
        root = pathlib.Path(directory)
        (root / 'scripts').mkdir()
        source = subprocess.check_output(['git', 'show', f'{revision}:scripts/install.sh'], cwd=REPO)
        (root / 'scripts/install.sh').write_bytes(source)
        Bootstrap.run_installer.__globals__['ROOT'] = root
        case = Bootstrap('test_default_uses_canonical_repository_and_legacy_release')
        case.setUp()
        try:
            case.routes['https://api.github.com/repos/scbrown/caboodle/releases/latest'] = {
                'status': 200, 'body': '{"tag_name":"v0.2.23"}\n'}
            case.archive('caboodle', 'v0.2.23', ['caboodle'])
            result = case.run_installer()
            installed = sorted(p.name for p in (case.root / 'installed').iterdir())
            print(f'{label} {revision}: default bootstrap exit={result.returncode}; installed={installed}', flush=True)
            for line in result.stdout.splitlines():
                if line.startswith('caboodle '):
                    print(line, flush=True)
            assert result.returncode == expected
            assert installed == ([] if label == 'BEFORE' else ['caboodle'])
        finally:
            case.doCleanups()
print('LIMIT: fixture URLs and generated executable archives; no host install or public release is claimed.', flush=True)
