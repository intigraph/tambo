#!/usr/bin/env python3
# /// script
# requires-python = ">=3.11"
# dependencies = []
# ///
"""Exercise bootstrap release selection with owned archives and fixture URLs."""
import hashlib
import io
import json
import os
from pathlib import Path
import subprocess
import sys
import tarfile
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
CURL = r'''import json, os, pathlib, sys
args = sys.argv[1:]
out = write = url = None
fail = False
while args:
    arg = args.pop(0)
    if arg in ('-o', '--output', '-w', '--write-out', '--connect-timeout', '--max-time'):
        value = args.pop(0)
        if arg in ('-o', '--output'): out = value
        if arg in ('-w', '--write-out'): write = value
    elif arg.startswith('-'):
        fail |= arg == '--fail' or (not arg.startswith('--') and 'f' in arg)
    else: url = arg
with open(os.environ['FIXTURE_REQUESTS'], 'a') as log: log.write(url + '\n')
route = json.loads(pathlib.Path(os.environ['FIXTURE_ROUTES']).read_text()).get(url, {'status': 404})
if route.get('exit'): sys.exit(route['exit'])
status = route['status']
if fail and status >= 400: sys.exit(22)
body = pathlib.Path(route['file']).read_bytes() if 'file' in route else route.get('body', '').encode()
if out: pathlib.Path(out).write_bytes(body)
else: sys.stdout.buffer.write(body)
if write: print(status, end='')
'''


class Bootstrap(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory(prefix='bootstrap-fixture-')
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name)
        for name in ('home', 'bin', 'tmp', 'installed'):
            (self.root / name).mkdir()
        curl = self.root / 'bin/curl'
        curl.write_text(f'#!{sys.executable}\n' + CURL)
        curl.chmod(0o755)
        self.routes = {}
        self.requests = self.root / 'requests'
        self.base = 'https://fixture.invalid/release'
        self.target = 'x86_64-unknown-linux-gnu'

    def archive(self, prefix, version, members, corrupt_checksum=False):
        name = f'{prefix}-{version}-{self.target}.tar.gz'
        path = self.root / name
        with tarfile.open(path, 'w:gz') as archive:
            for member in members:
                data = f'#!/bin/sh\necho "{member} {version[1:]} (fixture)"\n'.encode()
                info = tarfile.TarInfo(member)
                info.mode = 0o755
                info.size = len(data)
                archive.addfile(info, io.BytesIO(data))
        self.routes[f'{self.base}/{name}'] = {'status': 200, 'file': str(path)}
        checksum = '0' * 64 if corrupt_checksum else hashlib.sha256(path.read_bytes()).hexdigest()
        self.routes[f'{self.base}/{name}.sha256'] = {'status': 200, 'body': f'{checksum}  {name}\n'}

    def run_installer(self, version=None):
        route_file = self.root / 'routes.json'
        route_file.write_text(json.dumps(self.routes))
        env = {k: v for k, v in os.environ.items() if not k.startswith(('TAMBO_', 'CABOODLE_'))}
        env.update(HOME=str(self.root / 'home'), TMPDIR=str(self.root / 'tmp'),
                   PATH=f'{self.root / "bin"}:/usr/bin:/bin',
                   CABOODLE_INSTALL_DIR=str(self.root / 'installed'),
                   CABOODLE_TARGET=self.target, CABOODLE_RELEASE_BASE_URL=self.base,
                   FIXTURE_ROUTES=str(route_file), FIXTURE_REQUESTS=str(self.requests))
        if version:
            env['CABOODLE_VERSION'] = version
        self.result = subprocess.run(['bash', str(ROOT / 'scripts/install.sh')], env=env,
                                     capture_output=True, text=True, timeout=20)
        return self.result

    def assert_installed(self, names):
        self.assertEqual(self.result.returncode, 0, self.result.stdout + self.result.stderr)
        self.assertEqual(sorted(p.name for p in (self.root / 'installed').iterdir()), sorted(names))
        for name in names:
            self.assertTrue(os.access(self.root / 'installed' / name, os.X_OK))

    def test_default_uses_compatibility_repository_and_legacy_release(self):
        self.routes['https://api.github.com/repos/scbrown/caboodle/releases/latest'] = {
            'status': 200, 'body': '{"tag_name":"v0.2.23"}\n'}
        self.archive('caboodle', 'v0.2.23', ['caboodle'])
        self.run_installer()
        self.assert_installed(['caboodle'])
        self.assertIn('https://api.github.com/repos/scbrown/caboodle/releases/latest',
                      self.requests.read_text().splitlines())

    def test_pinned_legacy_release(self):
        self.archive('caboodle', 'v0.2.23', ['caboodle'])
        self.run_installer('v0.2.23')
        self.assert_installed(['caboodle'])
        self.assertNotIn('/repos/', self.requests.read_text())

    def test_new_archive_installs_both_commands(self):
        self.archive('tambo', 'v0.3.0', ['tambo', 'caboodle'])
        self.run_installer('v0.3.0')
        self.assert_installed(['tambo', 'caboodle'])

    def test_new_archive_requires_both_commands(self):
        for members in (['tambo'], ['caboodle']):
            with self.subTest(members=members):
                self.archive('tambo', 'v0.3.0', members)
                result = self.run_installer('v0.3.0')
                self.assertNotEqual(result.returncode, 0)
                self.assertEqual(list((self.root / 'installed').iterdir()), [])

    def test_bad_checksum_never_falls_back_or_replaces(self):
        self.archive('tambo', 'v0.3.0', ['tambo', 'caboodle'], corrupt_checksum=True)
        self.archive('caboodle', 'v0.3.0', ['caboodle'])
        existing = self.root / 'installed/caboodle'
        existing.write_bytes(b'prior installed bytes')
        self.assertNotEqual(self.run_installer('v0.3.0').returncode, 0)
        self.assertEqual(existing.read_bytes(), b'prior installed bytes')
        self.assertNotIn('/caboodle-', self.requests.read_text())

    def test_non_404_failure_never_falls_back(self):
        self.archive('caboodle', 'v0.3.0', ['caboodle'])
        for response in ({'status': 500}, {'status': 0, 'exit': 7}):
            with self.subTest(response=response):
                self.routes[f'{self.base}/tambo-v0.3.0-{self.target}.tar.gz'] = response
                self.assertNotEqual(self.run_installer('v0.3.0').returncode, 0)
                self.assertEqual(list((self.root / 'installed').iterdir()), [])
        self.assertNotIn('/caboodle-', self.requests.read_text())


if __name__ == '__main__':
    unittest.main(verbosity=2)
