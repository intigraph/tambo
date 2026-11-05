#!/usr/bin/env python3
"""Private packet controls: no dscl, root execution or account changes."""
import importlib.util
import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location('packet', Path(__file__).with_name('macos-job-account-packet.py'))
p = importlib.util.module_from_spec(spec)
spec.loader.exec_module(p)


class PacketTests(unittest.TestCase):
    def test_macos15_native_property_prefix(self):
        answer = type('Answer', (), {'stdout': 'UniqueID: 51001\ndsAttrTypeNative:IsHidden: 1\n'})()
        with patch.object(p, 'run', return_value=answer):
            self.assertEqual(p.properties('/Users/_stjobs', []),
                             {'UniqueID': '51001', 'IsHidden': '1'})
        for text in ('IsHidden: 1\ndsAttrTypeNative:IsHidden: 1\n',
                     'dsAttrTypeNative:IsHidden: 1\ndsAttrTypeNative:IsHidden: 0\n'):
            answer.stdout = text
            with patch.object(p, 'run', return_value=answer):
                with self.assertRaisesRegex(ValueError, 'duplicate'):
                    p.properties('/Users/_stjobs', [])
        answer.stdout = 'other:IsHidden: 1\n'
        with patch.object(p, 'run', return_value=answer):
            self.assertNotIn('IsHidden', p.properties('/Users/_stjobs', []))

    def test_directory_mode_under_private_umask(self):
        with tempfile.TemporaryDirectory() as raw, patch.object(p, 'protected'):
            old = os.umask(0o077)
            try:
                for name in ('base', 'base/bin', 'base/jobs'):
                    path = Path(raw) / name
                    p.directory(path)
                    self.assertEqual(path.stat().st_mode & 0o777, 0o755)
            finally:
                os.umask(old)

    def test_pinned_input_and_tamper(self):
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            tool = root / 'tambo'
            tool.write_bytes(b'fixture executable; never run')
            row = b'{"version":1}'
            principal = 'eph-task-123abc'
            (root / (principal + '.json')).write_bytes(row)
            catalog = json.dumps({'version': 1, 'slots': {principal: {'sha256': p.digest(row)}}}).encode()
            (root / 'slots.json').write_bytes(catalog)
            data = p.inputs(tool, p.digest(tool.read_bytes()), root, p.digest(catalog))
            self.assertEqual(len(data[1]), 2)
            (root / (principal + '.json')).write_bytes(b'changed')
            with self.assertRaisesRegex(ValueError, 'slot digest'):
                p.inputs(tool, p.digest(tool.read_bytes()), root, p.digest(catalog))
            with self.assertRaisesRegex(ValueError, 'tool digest'):
                p.inputs(tool, '0' * 64, root, p.digest(catalog))

    def test_traversal_rejected_before_slot_read(self):
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            tool = root / 'tool'
            tool.write_bytes(b'tool')
            catalog = json.dumps({'version': 1, 'slots': {'../../outside': {}}}).encode()
            (root / 'slots.json').write_bytes(catalog)
            with self.assertRaisesRegex(ValueError, 'principal'):
                p.inputs(tool, p.digest(b'tool'), root, p.digest(catalog))

    def test_no_symlink_input(self):
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            (root / 'real').write_bytes(b'bytes')
            (root / 'link').symlink_to(root / 'real')
            with self.assertRaises(OSError):
                p.regular(root / 'link')

    def test_create_repeat_and_occupied_preserved(self):
        with tempfile.TemporaryDirectory() as raw, patch.object(p, 'protected'):
            path = Path(raw) / 'destination'
            p.install(path, b'reviewed', 0o600)
            p.install(path, b'reviewed', 0o600)
            with self.assertRaisesRegex(ValueError, 'occupied'):
                p.install(path, b'new', 0o600)
            self.assertEqual(path.read_bytes(), b'reviewed')
            os.chmod(path, 0o644)
            with self.assertRaisesRegex(ValueError, 'occupied'):
                p.install(path, b'reviewed', 0o600)

    def test_wrong_platform_before_writes(self):
        with patch.object(p.sys, 'platform', 'linux'), patch.object(p, 'directory') as write:
            with self.assertRaisesRegex(ValueError, 'sudo on macOS'):
                p.execute(type('Args', (), {'mode': 'setup'})())
            write.assert_not_called()

    def test_unprivileged_before_writes(self):
        with patch.object(p.sys, 'platform', 'darwin'), patch.object(p.os, 'geteuid', return_value=501), patch.object(p, 'directory') as write:
            with self.assertRaisesRegex(ValueError, 'sudo on macOS'):
                p.execute(type('Args', (), {'mode': 'setup'})())
            write.assert_not_called()

    def test_duplicate_native_properties_refuse(self):
        answer = type('Answer', (), {'stdout': 'UniqueID: 51001\nUniqueID: 51002\n'})()
        with patch.object(p, 'run', return_value=answer):
            with self.assertRaisesRegex(ValueError, 'duplicate'):
                p.properties('/Users/_stjobs', ['UniqueID'])

    def test_native_verify_positive_and_wrong_uid(self):
        from types import SimpleNamespace
        row = dict(uid=51001, username='_stjobs', principal='eph-task-000001',
                   home='/var/lib/shantytown/agents/crew-eph-task-000001',
                   manifest_sha256='a' * 64)
        properties = dict(UniqueID='51001', PrimaryGroupID='51001',
                          NFSHomeDirectory=row['home'], UserShell='/usr/bin/false',
                          IsHidden='1', AuthenticationAuthority=';DisabledUser;')
        receipt = dict(principal=row['principal'], uid=51001,
                       sha256='a' * 64, state='complete')
        def native(args, ok=True):
            if args[0].endswith('dseditgroup'):
                return SimpleNamespace(returncode=1, stdout='no _stjobs is NOT a member of admin')
            return SimpleNamespace(returncode=0, stdout='_stjobs 51001')
        info = SimpleNamespace(st_mode=0o40700, st_uid=51001, st_gid=51001)
        with patch.object(p, 'protected'), patch.object(p, 'properties', return_value=properties), patch.object(p, 'run', side_effect=native), patch.object(p.Path, 'lstat', return_value=info), patch.object(p, 'regular', return_value=json.dumps(receipt).encode()):
            p.verify([row])
            info.st_uid = 501
            with self.assertRaisesRegex(ValueError, 'home ownership'):
                p.verify([row])

    def test_rollback_repeats_and_preserves_identity(self):
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            args = type('Args', (), {'mode': 'rollback'})()
            original_umask = os.umask(0o077)
            try:
                with patch.object(p.sys, 'platform', 'darwin'), patch.object(p.os, 'geteuid', return_value=0), patch.object(p, 'protected'), patch.object(p, 'JOBS', root), patch.object(p, 'STOP', root / 'stop'), patch.object(p, 'plan', return_value=[{'uid': 51001}]), patch.object(p, 'verify') as verify, patch.object(p, 'run') as native:
                    p.execute(args)
                    first = (root / 'stop').read_bytes()
                    p.execute(args)
                    self.assertEqual((root / 'stop').read_bytes(), first)
                    self.assertEqual(verify.call_count, 2)
                    native.assert_not_called()  # no stop/delete/account rewrite
            finally:
                os.umask(original_umask)

    def test_rollback_refuses_drift_before_tombstone(self):
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            original_umask = os.umask(0o077)
            try:
                with patch.object(p.sys, 'platform', 'darwin'), patch.object(p.os, 'geteuid', return_value=0), patch.object(p, 'protected'), patch.object(p, 'JOBS', root), patch.object(p, 'STOP', root / 'stop'), patch.object(p, 'plan', return_value=[]), patch.object(p, 'verify', side_effect=ValueError('drift')):
                    with self.assertRaisesRegex(ValueError, 'drift'):
                        p.execute(type('Args', (), {'mode': 'rollback'})())
                    self.assertFalse((root / 'stop').exists())
            finally:
                os.umask(original_umask)


if __name__ == '__main__':
    unittest.main()
