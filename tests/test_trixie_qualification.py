"""Privileged-job refusal controls; no package manager or compiler is invoked."""
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
ACTION = ROOT / '.github/actions/trixie-qualification/index.js'
spec = importlib.util.spec_from_file_location('trixie_package', ROOT / 'scripts/qualify-trixie-package.py')
package = importlib.util.module_from_spec(spec)
spec.loader.exec_module(package)


class FrozenInputs(unittest.TestCase):
    def node(self, script):
        result = subprocess.run(['node', '-e', f'const q=require({json.dumps(str(ACTION))});\n' + script],
                                capture_output=True, text=True, timeout=10)
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_matching_hash_and_size_pass_changed_bytes_or_size_refuse(self):
        expected = hashlib.sha256(b'original input').hexdigest()
        self.node(f'''const assert=require('node:assert/strict');
          q.verify(Buffer.from('original input'), '{expected}', 14);
          assert.throws(()=>q.verify(Buffer.from('altered input!'), '{expected}',14));
          assert.throws(()=>q.verify(Buffer.from('original input'), '{expected}',13));''')

    def test_release_requires_exact_section_path_size_and_digest(self):
        self.node('''const assert=require('node:assert/strict');
          const entry={name:'main/source/Sources.xz',sha256:'a'.repeat(64),bytes:42};
          const release=Buffer.from('SHA512:\\n fake\\nSHA256:\\n '+entry.sha256+' 42 '+entry.name+'\\nSHA1:\\n end');
          q.releaseEntry(release,entry);
          for(const changed of [{...entry,bytes:41},{...entry,name:'other'},{...entry,sha256:'b'.repeat(64)}])
            assert.throws(()=>q.releaseEntry(release,changed));
          assert.throws(()=>q.releaseEntry(Buffer.from('SHA512:\\n '+entry.sha256+' 42 '+entry.name),entry));''')

    def test_redirect_destination_and_protocol_are_rejected_before_network(self):
        self.node('''const assert=require('node:assert/strict');
          for(const url of ['http://snapshot.debian.org/file','https://example.invalid/file',
                            'https://snapshot.debian.org.evil.example/file'])
            assert.throws(()=>q.download(url,100));
          assert.throws(()=>q.download('https://snapshot.debian.org/file',100,6));''')

    def test_bootstrap_refuses_ordinary_host_before_apt_changes(self):
        directory = Path('/etc/apt/sources.list.d')
        before = {str(p): (p.stat().st_mtime_ns, hashlib.sha256(p.read_bytes()).hexdigest())
                  for p in directory.glob('*') if p.is_file()}
        run = subprocess.run(['node', str(ACTION)], env={**os.environ, 'GITHUB_ACTIONS': 'false'},
                             capture_output=True, text=True, timeout=10)
        self.assertNotEqual(run.returncode, 0)
        self.assertIn('disposable trixie job required', run.stderr)
        after = {str(p): (p.stat().st_mtime_ns, hashlib.sha256(p.read_bytes()).hexdigest())
                 for p in directory.glob('*') if p.is_file()}
        self.assertEqual(before, after)

    def test_frozen_manifest_has_both_indexes_each_pocket_and_no_dev_feature(self):
        manifest = json.loads((ROOT / 'scripts/trixie-qualification.json').read_text())
        self.assertEqual({s['suite'] for s in manifest['snapshots']},
                         {'trixie', 'trixie-updates', 'trixie-security'})
        for snapshot in manifest['snapshots']:
            self.assertEqual({i['name'] for i in snapshot['indexes']},
                             {'main/source/Sources.xz', 'main/binary-amd64/Packages.xz'})
            self.assertRegex(snapshot['release_sha256'], r'^[0-9a-f]{64}$')
        self.assertEqual(manifest['closure']['count'], len(manifest['closure']['packages']))
        self.assertNotIn('fixture-members', next(p['features'] for p in manifest['closure']['packages']
                                               if p['name'] == 'tambo'))

    def test_workflow_is_manual_single_job_read_only_and_short_lived(self):
        text = (ROOT / '.github/workflows/trixie-qualification.yml').read_text()
        self.assertEqual(text.split('\non:\n')[1].split('\npermissions:')[0], '  workflow_dispatch:')
        self.assertEqual(text.split('\npermissions:\n')[1].split('\nconcurrency:')[0], '  contents: read')
        self.assertEqual(text.count('    runs-on:'), 1)
        self.assertIn('debian:trixie@sha256:', text)
        self.assertIn('retention-days: 7', text)
        self.assertNotIn('secrets.', text)
        self.assertIn('test ! -e qualification-build', text)


class PackagePreparation(unittest.TestCase):
    def fixture(self, root):
        source = root / 'source'
        source.mkdir()
        (source / 'Cargo.toml').write_text('[package]\nversion="1.2.3"\n'
                                         '[[bin]]\nname="tambo"\n[[bin]]\nname="compat"\n')
        (source / 'Cargo.lock').write_text('retained immutable lock\n')
        (source / 'LICENSE').write_text('fixture terms\n')
        return source

    def test_both_bins_offline_locked_default_features_and_lock_are_preserved(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            source = self.fixture(root)
            before = package.digest(source / 'Cargo.lock')
            work = root / 'build'
            self.assertEqual(package.prepare(source, work), '1.2.3+qual1')
            rules = (work / 'debian/rules').read_text()
            self.assertIn('cargo build --offline --locked --release --bins', rules)
            self.assertNotIn('--features', rules)
            self.assertNotIn('--no-default-features', rules)
            for binary in ('tambo', 'compat'):
                self.assertIn(f'target/release/{binary} --version', rules)
                self.assertIn(f'debian/tambo/usr/bin/{binary}', rules)
            self.assertEqual(before, package.digest(work / 'Cargo.lock'))
            self.assertEqual(before, package.digest(source / 'Cargo.lock'))
            with self.assertRaisesRegex(ValueError, 'already exists'):
                package.prepare(source, work)

    def test_timeout_stops_command_before_returning_without_retry(self):
        import sys
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            pid = root / 'pid'
            command = [sys.executable, '-c',
                       "import os,time; open('pid','w').write(str(os.getpid())); time.sleep(30)"]
            self.assertEqual(package.run(command, root / 'timeout.log', root, timeout=0.3), 124)
            with self.assertRaises(ProcessLookupError):
                os.kill(int(pid.read_text()), 0)
            self.assertIn('no automatic retry', (root / 'timeout.log').read_text())

    def test_unexpected_source_binary_scope_refuses_before_copy(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            source = self.fixture(root)
            (source / 'Cargo.toml').write_text('[package]\nversion="1.2.3"\n[[bin]]\nname="tambo"\n')
            with self.assertRaisesRegex(ValueError, 'both declared'):
                package.prepare(source, root / 'build')
            self.assertFalse((root / 'build').exists())


if __name__ == '__main__':
    unittest.main()
