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
import shutil
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

    def run_installer(self, version=None, repository=None, legacy_repository=None):
        route_file = self.root / 'routes.json'
        route_file.write_text(json.dumps(self.routes))
        env = {k: v for k, v in os.environ.items() if not k.startswith(('TAMBO_', 'CABOODLE_'))}
        env.update(HOME=str(self.root / 'home'), TMPDIR=str(self.root / 'tmp'),
                   PATH=f'{self.root / "bin"}:/usr/bin:/bin',
                   CABOODLE_INSTALL_DIR=str(self.root / 'installed'),
                   CABOODLE_TARGET=self.target, CABOODLE_RELEASE_BASE_URL=self.base,
                   FIXTURE_ROUTES=str(route_file), FIXTURE_REQUESTS=str(self.requests))
        if repository is not None:
            env['TAMBO_REPOSITORY'] = repository
        if legacy_repository is not None:
            env['CABOODLE_REPOSITORY'] = legacy_repository
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

    def test_default_uses_canonical_repository_and_legacy_release(self):
        self.routes['https://api.github.com/repos/intigraph/tambo/releases/latest'] = {
            'status': 200, 'body': '{"tag_name":"v0.2.23"}\n'}
        self.archive('caboodle', 'v0.2.23', ['caboodle'])
        self.run_installer()
        self.assert_installed(['caboodle'])
        self.assertIn('https://api.github.com/repos/intigraph/tambo/releases/latest',
                      self.requests.read_text().splitlines())

    def test_legacy_repository_override_still_works(self):
        url = 'https://api.github.com/repos/scbrown/caboodle/releases/latest'
        self.routes[url] = {'status': 200, 'body': '{"tag_name":"v0.2.23"}'}
        self.archive('caboodle', 'v0.2.23', ['caboodle'])
        self.run_installer(legacy_repository='scbrown/caboodle')
        self.assert_installed(['caboodle'])
        self.assertIn(url, self.requests.read_text().splitlines())

    def test_tambo_repository_override_wins_over_legacy(self):
        url = 'https://api.github.com/repos/fixture-owner/tambo/releases/latest'
        self.routes[url] = {'status': 200, 'body': '{"tag_name":"v0.2.23"}'}
        self.archive('caboodle', 'v0.2.23', ['caboodle'])
        self.run_installer(repository='fixture-owner/tambo', legacy_repository='scbrown/caboodle')
        self.assert_installed(['caboodle'])
        self.assertIn(url, self.requests.read_text().splitlines())
        self.assertNotIn('repos/scbrown/caboodle', self.requests.read_text())

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

    def test_root_installer_matches_compatibility_entrypoint(self):
        self.assertEqual((ROOT / 'install.sh').read_bytes(),
                         (ROOT / 'scripts/install.sh').read_bytes())

    def test_unsupported_platform_preserves_existing_install(self):
        self.target = 'aarch64-unknown-linux-gnu'
        existing = self.root / 'installed/tambo'
        existing.write_bytes(b'prior installed bytes')
        self.assertNotEqual(self.run_installer('v0.4.0').returncode, 0)
        self.assertIn('unsupported', self.result.stderr)
        self.assertEqual(existing.read_bytes(), b'prior installed bytes')

    def test_missing_assets_preserve_existing_install(self):
        existing = self.root / 'installed/tambo'
        existing.write_bytes(b'prior installed bytes')
        self.assertNotEqual(self.run_installer('v0.4.0').returncode, 0)
        self.assertEqual(existing.read_bytes(), b'prior installed bytes')

    def test_wrong_version_preserves_existing_install(self):
        self.archive('tambo', 'v0.3.0', ['tambo', 'caboodle'])
        for url, route in list(self.routes.items()):
            self.routes[url.replace('v0.3.0', 'v0.4.0')] = route
        # Rewrite only the checksum filename: verified bytes still report 0.3.0.
        url = f'{self.base}/tambo-v0.4.0-{self.target}.tar.gz.sha256'
        self.routes[url]['body'] = self.routes[url]['body'].replace('v0.3.0', 'v0.4.0')
        existing = self.root / 'installed/tambo'
        existing.write_bytes(b'prior installed bytes')
        self.assertNotEqual(self.run_installer('v0.4.0').returncode, 0)
        self.assertIn('version mismatch', self.result.stderr)
        self.assertEqual(existing.read_bytes(), b'prior installed bytes')

    def test_non_404_failure_never_falls_back(self):
        self.archive('caboodle', 'v0.3.0', ['caboodle'])
        for response in ({'status': 500}, {'status': 0, 'exit': 7}):
            with self.subTest(response=response):
                self.routes[f'{self.base}/tambo-v0.3.0-{self.target}.tar.gz'] = response
                self.assertNotEqual(self.run_installer('v0.3.0').returncode, 0)
                self.assertEqual(list((self.root / 'installed').iterdir()), [])
        self.assertNotIn('/caboodle-', self.requests.read_text())


class CloudEnvironment(unittest.TestCase):
    """Run the whole cloud setup with private tools and no network access."""

    def setUp(self):
        Bootstrap.setUp(self)
        self.url = 'https://raw.githubusercontent.com/intigraph/seeds/main/install.sh'
        self.calls = self.root / 'tool-calls'
        # An ambient sd must not satisfy the "missing" fixture. Copies, not
        # symlinks, keep later stubbing isolated from the real executables.
        for tool in ('bash', 'mkdir', 'sh', 'head', 'cat', 'chmod', 'python3', 'timeout'):
            target = self.root / 'bin' / tool
            shutil.copyfile(shutil.which(tool), target)
            target.chmod(0o755)
        for tool in ('just', 'pre-commit', 'mdbook', 'mdbook-mermaid',
                     'quipu', 'quipu-server', 'caboodle'):
            self.executable(tool, f'#!/bin/sh\necho "{tool} fixture"\n')
        for tool in ('npm', 'pip', 'cargo'):
            self.executable(tool, '#!/bin/sh\nprintf "%s\\n" "$0 $*" >> "$FIXTURE_CALLS"\nexit 1\n')

    def executable(self, name, body):
        path = self.root / 'bin' / name
        path.write_text(body)
        path.chmod(0o755)

    def seeds(self, identity, help_status=0):
        return ('#!/bin/sh\ncase "$1" in\n'
                f'--help) echo "{identity}"; exit {help_status};;\n'
                '--version) echo "sd 0.0.0 (fixture seeds)";;\n'
                '*) echo "unexpected board command" >&2; exit 64;;\nesac\n')

    def run_setup(self, download=True, install=True):
        body = ('#!/bin/sh\ncat > "$HOME/.local/bin/sd" <<\'SD\'\n'
                + self.seeds('seeds is the work-item tracker for the quipu stack')
                + 'SD\nchmod +x "$HOME/.local/bin/sd"\n') if install else 'exit 1\n'
        routes = self.root / 'routes.json'
        routes.write_text(json.dumps({self.url: {'status': 200 if download else 503,
                                                'body': body}}))
        env = {k: v for k, v in os.environ.items()
               if not k.startswith(('TAMBO_', 'CABOODLE_'))}
        env.update(HOME=str(self.root / 'home'), TMPDIR=str(self.root / 'tmp'),
                   PATH=str(self.root / 'bin'),
                   FIXTURE_ROUTES=str(routes), FIXTURE_REQUESTS=str(self.requests),
                   FIXTURE_CALLS=str(self.calls))
        result = subprocess.run(['bash', str(ROOT / 'scripts/setup-environment.sh')],
                                env=env, capture_output=True, text=True, timeout=20)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        calls = self.calls.read_text() if self.calls.exists() else ''
        self.assertNotIn('npm', calls)
        self.assertNotIn('cargo', calls)
        return result

    def test_missing_seeds_installs_release_instead_of_beads(self):
        result = self.run_setup()
        self.assertIn('sd 0.0.0 (fixture seeds)', result.stdout)
        self.assertEqual(self.requests.read_text().splitlines(), [self.url])
        self.assertFalse((self.root / 'home/.seeds').exists())

    def test_both_known_seeds_identities_skip_installation(self):
        for identity in ('seeds is the beads-compatible tracker',
                         'seeds is the work-item tracker for the quipu stack'):
            with self.subTest(identity=identity):
                self.executable('sd', self.seeds(identity))
                self.assertIn('sd 0.0.0 (fixture seeds)', self.run_setup().stdout)
                self.assertFalse(self.requests.exists())

    def test_unrelated_sd_does_not_satisfy_dependency(self):
        self.executable('sd', self.seeds('sd is a find and replace tool'))
        result = self.run_setup()
        self.assertIn('sd 0.0.0 (fixture seeds)', result.stdout)
        self.assertEqual(self.requests.read_text().splitlines(), [self.url])

    def test_failed_help_is_not_an_identity_proof(self):
        self.executable('sd', self.seeds('seeds is the beads-compatible tracker', 1))
        self.run_setup()
        self.assertEqual(self.requests.read_text().splitlines(), [self.url])

    def test_failed_optional_install_reports_missing(self):
        for download, install in ((False, True), (True, False)):
            with self.subTest(download=download, install=install):
                result = self.run_setup(download=download, install=install)
                self.assertRegex(result.stdout, r'sd\s+MISSING')

    def test_failed_install_cannot_report_unrelated_sd_as_seeds(self):
        self.executable('sd', self.seeds('sd is a find and replace tool'))
        result = self.run_setup(download=False)
        self.assertRegex(result.stdout, r'sd\s+UNRECOGNIZED \(not Seeds\)')

    def test_timed_out_install_keeps_the_session_available(self):
        self.executable('timeout', '#!/bin/sh\nexit 124\n')
        self.assertRegex(self.run_setup().stdout, r'sd\s+MISSING')


if __name__ == '__main__':
    unittest.main(verbosity=2)
