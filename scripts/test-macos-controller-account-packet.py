#!/usr/bin/env python3
"""Private native-command controls; no real dscl, accounts, sudo or host mutation."""
import hashlib
import importlib.util
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location('controller_packet', Path(__file__).with_name('macos-controller-account-packet.py'))
p = importlib.util.module_from_spec(spec)
spec.loader.exec_module(p)
ROOT = Path(__file__).resolve().parents[1]
PACKET = ROOT / 'docs/operator-packets/e579/controller.json'
CATALOG = ROOT / 'docs/operator-packets/e579/catalog'
PIN = hashlib.sha256(PACKET.read_bytes()).hexdigest()
CATALOG_PIN = hashlib.sha256((CATALOG / 'slots.json').read_bytes()).hexdigest()
ROW, _ = p.packet(PACKET, PIN)


class ControllerTests(unittest.TestCase):
    def test_cli_help_and_unprivileged_offline_verify_python39(self):
        for arguments in [['--help'], ['verify', '--offline', '--packet', str(PACKET),
                '--packet-sha256', PIN, '--inventory', str(CATALOG), '--inventory-sha256', CATALOG_PIN]]:
            run = subprocess.run([sys.executable, '-I', str(Path(p.__file__)), *arguments],
                                 capture_output=True, text=True, timeout=20)
            self.assertEqual(run.returncode, 0, run.stderr)
        self.assertIn('native controller/readiness UNPROVED', run.stdout)

    def test_refuses_low_uid_job_uid_and_packet_tamper(self):
        with tempfile.TemporaryDirectory() as raw:
            path = Path(raw) / 'controller.json'
            for uid in [501, 9999, 51001, True, '51000']:
                path.write_text(json.dumps(dict(ROW, uid=uid)))
                with self.subTest(uid=uid), self.assertRaisesRegex(ValueError, 'controller UID'):
                    p.packet(path, p.digest(path.read_bytes()))
            with self.assertRaisesRegex(ValueError, 'digest'):
                p.packet(PACKET, '0' * 64)

    def test_catalog_must_pin_same_controller(self):
        p.catalog(CATALOG, CATALOG_PIN, 51000)
        with self.assertRaisesRegex(ValueError, 'controller UID differs'):
            p.catalog(CATALOG, CATALOG_PIN, 51002)
        with self.assertRaisesRegex(ValueError, 'catalog digest'):
            p.catalog(CATALOG, '0' * 64, 51000)

    def test_native_collisions_before_mutation(self):
        for table in [{'foreign': 51000}, {'_stctl': 52000}]:
            with tempfile.TemporaryDirectory() as raw, patch.object(p, 'RECEIPT', Path(raw) / 'receipt'), \
                    patch.object(p, 'STOP', Path(raw) / 'stop'), \
                    patch.object(p, 'inventories', return_value=(table, {'wheel': 0})), \
                    patch.object(p, 'run') as native, patch.object(p, 'install') as write:
                with self.assertRaisesRegex(ValueError, 'collision'):
                    p.setup(ROW, PIN, b'pinned tool')
                native.assert_not_called()
                write.assert_not_called()

    def test_complete_rerun_is_readonly_and_prepared_refuses(self):
        with tempfile.TemporaryDirectory() as raw, patch.object(p, 'STOP', Path(raw) / 'stop'), \
                patch.object(p, 'RECEIPT', Path(raw) / 'receipt'), patch.object(p, 'protected'), \
                patch.object(p, 'native_verify') as verify, patch.object(p, 'run') as native, patch.object(p, 'TOOL', Path(raw) / 'tool'):
            p.RECEIPT.write_text(json.dumps(p.receipt(ROW, PIN, 'complete')))
            p.RECEIPT.chmod(0o600)
            p.TOOL.write_bytes(b'pinned tool')
            self.assertIn('UNCHANGED', p.setup(ROW, PIN, b'pinned tool'))
            verify.assert_called_once_with(ROW)
            native.assert_not_called()
            p.RECEIPT.write_text(json.dumps(p.receipt(ROW, PIN, 'prepared')))
            with self.assertRaisesRegex(ValueError, 'explicit recovery'):
                p.setup(ROW, PIN)

    def test_failed_creation_retains_prepared_nonreuse_receipt(self):
        with tempfile.TemporaryDirectory() as raw, patch.object(p, 'STATE', Path(raw)), \
                patch.object(p, 'RECEIPT', Path(raw) / 'receipt'), \
                patch.object(p, 'STOP', Path(raw) / 'stop'), patch.object(p, 'TOOL', Path(raw) / 'tool'), \
                patch.object(p, 'protected'), patch.object(p, 'directory'), \
                patch.object(p, 'inventories', return_value=({'root': 0}, {'wheel': 0})), \
                patch.object(p, 'run') as native:
            native.side_effect = [type('Reply', (), {'stdout': json.dumps({'accounts': [{'controller_uid': 51000, 'uid': 51001}]})})(), ValueError('native failure')]
            (Path(raw) / 'catalog').mkdir()
            with self.assertRaisesRegex(ValueError, 'native failure'):
                p.setup(ROW, PIN, b'pinned fixture tool never executed')
            self.assertEqual(p.read_receipt(), p.receipt(ROW, PIN, 'prepared'))
            with self.assertRaisesRegex(ValueError, 'explicit recovery'):
                p.setup(ROW, PIN, b'pinned fixture tool never executed')

    def test_prepared_complete_atomic_and_wrong_packet_preserved(self):
        with tempfile.TemporaryDirectory() as raw, patch.object(p, 'STATE', Path(raw)), \
                patch.object(p, 'RECEIPT', Path(raw) / 'receipt'), patch.object(p, 'protected'):
            p.RECEIPT.write_text(json.dumps(p.receipt(ROW, PIN, 'prepared')))
            p.RECEIPT.chmod(0o600)
            p.save_complete(ROW, PIN)
            self.assertEqual(p.read_receipt(), p.receipt(ROW, PIN, 'complete'))
            before = p.RECEIPT.read_bytes()
            with self.assertRaisesRegex(ValueError, 'reservation changed'):
                p.save_complete(ROW, '0' * 64)
            self.assertEqual(p.RECEIPT.read_bytes(), before)

    def test_tombstone_refuses_reapply_and_preserves_receipt(self):
        with tempfile.TemporaryDirectory() as raw, patch.object(p, 'STOP', Path(raw) / 'stop'), \
                patch.object(p, 'RECEIPT', Path(raw) / 'receipt'), patch.object(p, 'run') as native:
            p.STOP.write_text('rolled back')
            p.RECEIPT.write_text('retained')
            with self.assertRaisesRegex(ValueError, 'tombstone'):
                p.setup(ROW, PIN)
            self.assertEqual(p.RECEIPT.read_text(), 'retained')
            native.assert_not_called()

    def test_unique_inventory_and_macos15_property_prefix(self):
        p.unique(ROW, {'_stctl': 51000}, {'_stctl': 51000})
        for users, groups in [({'_stctl': 51000, 'collision': 51000}, {'_stctl': 51000}),
                              ({'_stctl': 51000}, {'wrong': 51000})]:
            with self.assertRaisesRegex(ValueError, 'not unique'):
                p.unique(ROW, users, groups)
        reply = type('Reply', (), {'stdout': 'UniqueID: 51000\ndsAttrTypeNative:IsHidden: 1\n'})()
        with patch.object(p, 'run', return_value=reply):
            self.assertEqual(p.properties('/Users/_stctl', []), {'UniqueID': '51000', 'IsHidden': '1'})
            reply.stdout += 'IsHidden: 1\n'
            with self.assertRaisesRegex(ValueError, 'duplicate'):
                p.properties('/Users/_stctl', [])

    def test_native_verify_fields_groups_and_private_home(self):
        from types import SimpleNamespace
        expected = {'UniqueID': '51000', 'PrimaryGroupID': '51000',
                    'NFSHomeDirectory': ROW['home'], 'UserShell': '/usr/bin/false',
                    'IsHidden': '1', 'AuthenticationAuthority': ';DisabledUser;'}
        groups = {'_stctl': '51000', 'login': '20 80'}
        info = SimpleNamespace(st_mode=0o40700, st_uid=51000, st_gid=51000)
        home = SimpleNamespace(parent=Path('/nonexistent-parent'), lstat=lambda: info)
        def native(args, ok=True):
            if args[0] == '/usr/sbin/dseditgroup':
                return SimpleNamespace(returncode=1, stdout='no _stctl is NOT a member of admin')
            return SimpleNamespace(returncode=0, stdout=groups[args[-1]])
        with patch.object(p, 'inventories', return_value=({'_stctl': 51000, 'login': 501}, {'_stctl': 51000})), \
                patch.object(p, 'properties', return_value=expected), patch.object(p, 'run', side_effect=native), \
                patch.object(p, 'Path', return_value=home), patch.object(p, 'protected'):
            p.native_verify(ROW)
            groups['_stctl'] = '51000 20'
            with self.assertRaisesRegex(ValueError, 'shares a login group'):
                p.native_verify(ROW)
            groups['_stctl'] = '51000'
            info.st_mode = 0o40755
            with self.assertRaisesRegex(ValueError, 'private home differs'):
                p.native_verify(ROW)
            info.st_mode = 0o40700
            expected['UserShell'] = '/bin/sh'
            with self.assertRaisesRegex(ValueError, 'identity/login'):
                p.native_verify(ROW)

    def test_rollback_is_idempotent_and_drift_refuses(self):
        from types import SimpleNamespace
        args = SimpleNamespace(mode='rollback', packet=PACKET, packet_sha256=PIN,
            inventory=CATALOG, inventory_sha256=CATALOG_PIN, offline=False, tool_sha256=p.digest(b'pinned tool'))
        with tempfile.TemporaryDirectory() as raw, patch.object(p, 'BASE', Path(raw)), \
                patch.object(p, 'STATE', Path(raw)), patch.object(p, 'STOP', Path(raw) / 'stop'), \
                patch.object(p, 'RECEIPT', Path(raw) / 'receipt'), patch.object(p, 'protected'), \
                patch.object(p, 'native_verify'), patch.object(p, 'TOOL', Path(raw) / 'tool'), patch.object(p.sys, 'platform', 'darwin'), \
                patch.object(p.os, 'geteuid', return_value=0):
            p.RECEIPT.write_text(json.dumps(p.receipt(ROW, PIN, 'complete')))
            p.RECEIPT.chmod(0o600)
            p.TOOL.write_bytes(b'pinned tool')
            retained = p.RECEIPT.read_bytes()
            self.assertIn('ROLLED BACK', p.execute(args))
            before = p.STOP.read_bytes()
            self.assertIn('ROLLED BACK', p.execute(args))
            self.assertEqual(p.RECEIPT.read_bytes(), retained)
            p.RECEIPT.write_text('{}')
            with self.assertRaisesRegex(ValueError, 'not proved'):
                p.execute(args)
            self.assertEqual(p.STOP.read_bytes(), before)

    def test_regular_input_refuses_symlink(self):
        with tempfile.TemporaryDirectory() as raw:
            link = Path(raw) / 'link'
            link.symlink_to(PACKET)
            with self.assertRaises(OSError):
                p.packet(link, PIN)


if __name__ == '__main__':
    unittest.main()
