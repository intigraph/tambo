#!/usr/bin/env python3
"""Disposable filesystem controls; never invokes sudo or native account commands."""
import fcntl
import datetime as dt
import importlib.util
import json
import os
from pathlib import Path
import stat
import tempfile
import types
import unittest
from unittest.mock import patch

SPEC = importlib.util.spec_from_file_location('transaction', Path(__file__).with_name('macos-e579-runtime-transaction.py'))
m = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(m)


class Fixture(m.Transaction):
    def safe(self, path, mode=None):
        # Fixture-only trust root; production CLI cannot select this implementation.
        for p in (path, *path.parents):
            if p == self.fixture.parent:
                break
            info = p.lstat()
            if info.st_uid != os.getuid() or info.st_mode & 0o022 or not (
                    stat.S_ISDIR(info.st_mode) or stat.S_ISREG(info.st_mode)):
                raise ValueError('unsafe fixture path')
        info = path.lstat()
        if info.st_nlink != 1 and stat.S_ISREG(info.st_mode):
            raise ValueError('hardlink')
        if mode is not None and stat.S_IMODE(info.st_mode) != mode:
            raise ValueError('fixture mode differs')


class Controls(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        for n in ('base/bin', 'base/jobs/account-receipts', 'base/controllers', 'opt', 'helper', 'policy', 'inputs'):
            (self.root / n).mkdir(parents=True, exist_ok=True)
        for p in self.root.rglob('*'):
            if p.is_dir(): p.chmod(0o755)
        self.tx = Fixture(self.root / 'base', self.root / 'opt', self.root / 'helper/launcher', self.root / 'policy/st-job-slots', os.getuid())
        self.tx.fixture = self.root
        self.old_tool, self.new_tool = b'old-tool', b'qualified-new-tool'
        self.old_files = {'runtime/bin/python3': b'old-python', 'runtime/app/module.py': b'old-module', 'launcher': b'fixed-launcher'}
        self.new_files = {'runtime/bin/python3': b'new-python', 'runtime/app/module.py': b'new-module', 'runtime/app/catalog.py': b'new-catalog', 'launcher': b'fixed-launcher'}
        def manifest(files):
            return json.dumps(dict(version=1, platform='macos', files={n: m.sha(b) for n, b in files.items()}), sort_keys=True).encode()
        self.old_manifest, self.new_manifest = manifest(self.old_files), manifest(self.new_files)
        values = dict(OLD_TOOL=m.sha(self.old_tool), NEW_TOOL=m.sha(self.new_tool), OLD_RUNTIME=m.sha(self.old_manifest), NEW_RUNTIME=m.sha(self.new_manifest), HELPER=m.sha(b'fixed-launcher'), CATALOG=m.sha(b'catalog'), JOB=m.sha(b'identity-job'))
        for name, value in values.items():
            p = patch.object(m, name, value)
            p.start(); self.addCleanup(p.stop)
        self.write(self.tx.tool, self.old_tool, 0o755)
        self.write(self.tx.helper, b'fixed-launcher', 0o755)
        self.write(self.tx.base / 'jobs/slots.json', b'catalog')
        self.write(self.tx.base / 'jobs/eph-task-000001.json', b'identity-job')
        self.write(self.tx.base / 'controllers/51000.json', json.dumps(dict(version=1, uid=51000, packet_sha256=m.CONTROLLER, state='complete')).encode(), 0o600)
        self.write(self.tx.base / 'jobs/account-receipts/51001.json', json.dumps(dict(principal='eph-task-000001', uid=51001, sha256=m.JOB, state='complete')).encode(), 0o600)
        self.tx.runtime.mkdir()
        for n, raw in self.old_files.items():
            if n.startswith('runtime/'):
                self.write(self.tx.runtime / n[8:], raw, 0o755 if n.startswith('runtime/bin/') else 0o644)
        self.write(self.tx.runtime / '.artifact.json', self.old_manifest)
        self.tool = self.root / 'inputs/tambo'
        self.write(self.tool, self.new_tool, 0o755)
        self.archive = self.root / 'inputs/runtime.tar.gz'
        self.write(self.archive, b'verified-by-stage-fixture')
        self.stage = types.SimpleNamespace(read_archive=lambda *args: {**self.new_files, 'manifest.json': self.new_manifest})

    def write(self, path, raw, mode=0o644):
        path.parent.mkdir(parents=True, exist_ok=True)
        for p in path.parents:
            if p == self.root: break
            p.chmod(0o755)
        path.write_bytes(raw)
        path.chmod(mode)

    def test_a0_is_read_only(self):
        before = sorted(str(p.relative_to(self.root)) for p in self.root.rglob('*'))
        result = self.tx.observe()
        self.assertFalse(result['native_ready'])
        self.assertEqual(before, sorted(str(p.relative_to(self.root)) for p in self.root.rglob('*')))

    def test_full_forward_and_inverse_preserves_both_versions(self):
        self.tx.a1(self.tool)
        self.assertEqual(self.tx.tool.read_bytes(), self.new_tool)
        self.tx.a2(self.archive, self.stage)
        self.tx.tree(self.tx.runtime, m.NEW_RUNTIME)
        self.tx.a3()
        self.tx.tree(self.tx.runtime, m.OLD_RUNTIME)
        self.tx.tree(self.tx.failed_tree, m.NEW_RUNTIME)
        self.assertEqual(self.tx.tool.read_bytes(), self.old_tool)
        self.assertEqual((self.tx.state / 'retained-new-tool').read_bytes(), self.new_tool)

    def test_a1_does_not_replay(self):
        self.tx.a1(self.tool)
        with self.assertRaises(ValueError):
            self.tx.a1(self.tool)

    def test_wrong_new_tool_preserves_preimage(self):
        self.tool.write_bytes(b'wrong')
        with self.assertRaises(ValueError): self.tx.a1(self.tool)
        self.assertEqual(self.tx.tool.read_bytes(), self.old_tool)
        self.assertFalse(self.tx.state.exists())

    def test_prepared_account_refuses_before_backup(self):
        path = self.tx.base / 'controllers/51000.json'
        row = json.loads(path.read_bytes()); row['state'] = 'prepared'
        self.write(path, json.dumps(row).encode(), 0o600)
        with self.assertRaises(ValueError): self.tx.a1(self.tool)
        self.assertFalse(self.tx.state.exists())

    def test_runtime_drift_refuses(self):
        (self.tx.runtime / 'app/module.py').write_bytes(b'drift')
        with self.assertRaises(ValueError): self.tx.observe()

    def test_extra_runtime_file_refuses(self):
        self.write(self.tx.runtime / 'extra', b'extra')
        with self.assertRaises(ValueError): self.tx.observe()

    def test_symlink_refuses(self):
        self.tx.tool.unlink(); self.tx.tool.symlink_to(self.tool)
        with self.assertRaises(ValueError): self.tx.observe()

    def test_concurrent_writer_refuses(self):
        lock = self.tx.base / '.controller-packet-lock'
        self.write(lock, b'', 0o600)
        with lock.open('rb') as f:
            fcntl.flock(f.fileno(), fcntl.LOCK_EX | fcntl.LOCK_NB)
            with self.assertRaises(BlockingIOError): self.tx.a1(self.tool)
        self.assertFalse(self.tx.state.exists())

    def test_native_observer_failure_stops_a1(self):
        def unknown(): raise ValueError('root process instrument UNKNOWN')
        self.tx.native = unknown
        with self.assertRaises(ValueError): self.tx.a1(self.tool)
        self.assertEqual(self.tx.tool.read_bytes(), self.old_tool)

    def test_phase_use_blocks_inverse(self):
        self.tx.a1(self.tool); self.tx.a2(self.archive, self.stage)
        self.write(self.tx.base / 'jobs/used', b'used', 0o600)
        with self.assertRaises(ValueError): self.tx.a3()
        self.tx.tree(self.tx.runtime, m.NEW_RUNTIME)

    def test_mutated_protected_backup_blocks_inverse(self):
        self.tx.a1(self.tool)
        (self.tx.proof / 'app/module.py').write_bytes(b'bad backup')
        with self.assertRaises(ValueError): self.tx.a3()
        self.assertEqual(self.tx.tool.read_bytes(), self.new_tool)

    def test_failure_after_old_rename_can_only_inverse(self):
        self.tx.a1(self.tool)
        rename = os.rename
        def fail_once(src, dst):
            if src == self.tx.pending and dst == self.tx.runtime: raise OSError('injected rename failure')
            return rename(src, dst)
        with patch.object(m.os, 'rename', fail_once):
            with self.assertRaises(OSError): self.tx.a2(self.archive, self.stage)
        self.assertFalse(self.tx.runtime.exists())
        self.tx.tree(self.tx.old_tree, m.OLD_RUNTIME)
        with self.assertRaises((OSError, ValueError)): self.tx.a2(self.archive, self.stage)
        self.tx.a3()
        self.tx.tree(self.tx.runtime, m.OLD_RUNTIME)
        self.assertTrue(self.tx.pending.exists())  # No cleanup of interrupted output.

    def test_new_runtime_corruption_blocks_inverse(self):
        self.tx.a1(self.tool); self.tx.a2(self.archive, self.stage)
        (self.tx.runtime / 'app/module.py').write_bytes(b'unknown')
        with self.assertRaises(ValueError): self.tx.a3()
        self.assertTrue(self.tx.old_tree.exists())

    def test_a2_requires_completed_tool_phase(self):
        with self.assertRaises((OSError, ValueError)): self.tx.a2(self.archive, self.stage)
        self.tx.tree(self.tx.runtime, m.OLD_RUNTIME)

    def native_fixture(self, override=None):
        replies = {
            '/bin/launchctl': (0, 'system = {\n}', ''),
            '/usr/bin/sudo': (1, 'User _stctl is not allowed to run sudo on fixture.\n', ''),
            '/bin/ps': (0, '0 1 /sbin/launchd\n', ''),
            '/usr/sbin/lsof': (0, 'p1\nu0\nn/sbin/launchd\n', '')}
        replies.update(override or {})
        def run(args, ok=True):
            code, out, err = replies[args[0]]
            if args[0] == '/usr/bin/sudo' and '/usr/bin/sudo' not in (override or {}):
                out = 'User ' + args[-1] + ' is not allowed to run sudo on fixture.\n'
            if ok and code: raise ValueError('native read failed')
            return types.SimpleNamespace(returncode=code, stdout=out, stderr=err)
        return types.SimpleNamespace(native_verify=lambda row: None, inventories=lambda: ({'_stctl': 51000, '_stjobs': 51001}, {}), run=run)

    def test_native_readonly_instruments_positive(self):
        with patch.object(m, 'load_module', return_value=self.native_fixture()):
            m.native_observer(self.tx)

    def test_changed_a0_binding_refuses_before_replacement(self):
        with self.assertRaises(ValueError): self.tx.a1(self.tool, '0' * 64)
        self.assertEqual(self.tx.tool.read_bytes(), self.old_tool)
        self.assertFalse(self.tx.state.exists())

    def test_native_instruments_fail_closed(self):
        cases = [
            {'/bin/ps': (0, '51001 7 job\n', '')},
            {'/bin/ps': (0, '', '')},
            {'/usr/sbin/lsof': (0, 'p1\nu51000\n', '')},
            {'/usr/sbin/lsof': (0, 'p1\nuUNKNOWN\n', '')},
            {'/usr/sbin/lsof': (0, 'p1\nu0\nn' + str(self.tx.runtime) + '/bin/python3\n', '')},
            {'/usr/sbin/lsof': (0, 'p1\nu0\n', 'coverage warning')},
            {'/usr/sbin/lsof': (1, '', '')},
            {'/usr/bin/sudo': (1, '', 'authentication unknown')},
            {'/usr/bin/sudo': (0, '(root) NOPASSWD: ALL', '')},
            {'/bin/launchctl': (0, '', '')},
            {'/bin/launchctl': (0, 'system = { org.shantytown.controller }', '')}]
        for case in cases:
            with self.subTest(case=case), patch.object(m, 'load_module', return_value=self.native_fixture(case)):
                with self.assertRaises(ValueError): m.native_observer(self.tx)


class AdmissionControls(unittest.TestCase):
    def setUp(self):
        self.now = dt.datetime(2026, 1, 1, tzinfo=dt.timezone.utc)
        self.packet = dict(version=1, transaction='e579-0141', phase='a1',
                           expires_at='2026-01-01T01:00:00Z', freeze='all-job-admission-disabled',
                           a0_sha256='1' * 64, tool='/protected/tool', archive='/protected/runtime.tar.gz',
                           sources={name: '2' * 64 for name in ('macos-e579-runtime-transaction.py', 'macos-controller-account-packet.py', 'macos-job-runtime-stage.py')})

    def validate(self, packet):
        raw = json.dumps(packet).encode()
        return m.admission(raw, m.sha(raw), 'a1', self.now)

    def test_exact_admission(self):
        self.assertEqual(self.validate(self.packet), self.packet)

    def test_expired_or_unbounded(self):
        for expiry in ('2025-12-31T23:59:59Z', '2026-01-01T03:00:00Z', '2026-01-01T01:00:00'):
            with self.subTest(expiry=expiry), self.assertRaises(ValueError):
                self.validate({**self.packet, 'expires_at': expiry})

    def test_scope_and_pins(self):
        for change in ({'phase': 'a2'}, {'freeze': 'unknown'}, {'sources': {}},
                       {'a0_sha256': ''}, {'tool': 'relative'}, {'extra': True}):
            with self.subTest(change=change), self.assertRaises(ValueError):
                self.validate({**self.packet, **change})

    def test_changed_bytes(self):
        raw = json.dumps(self.packet).encode()
        with self.assertRaises(ValueError): m.admission(raw, '0' * 64, 'a1', self.now)

    def test_duplicate_key(self):
        raw = json.dumps(self.packet).replace('"version": 1', '"version": 1, "version": 1').encode()
        with self.assertRaises(ValueError): m.admission(raw, m.sha(raw), 'a1', self.now)


if __name__ == '__main__':
    unittest.main()
