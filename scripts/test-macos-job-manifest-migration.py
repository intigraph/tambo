#!/usr/bin/env python3
"""Private filesystem controls; never accounts, keys, sudo or a native launch."""
import importlib.util
import json
import sqlite3
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parent.parent
spec = importlib.util.spec_from_file_location('migration', ROOT / 'scripts/macos-job-manifest-migration.py')
m = importlib.util.module_from_spec(spec)
spec.loader.exec_module(m)


class Migration(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.base = Path(self.temp.name)
        self.old = (ROOT / 'docs/operator-packets/e579/catalog/eph-task-000001.json').read_bytes()
        self.plan = (ROOT / 'docs/operator-packets/e579/catalog/slots.json').read_bytes()
        home = json.loads(self.old)['home']
        self.extra = {'controller_root': '/var/lib/shantytown/controllers/_stctl/.shanty',
                      'workspace': home + '/work', 'settings': home + '/settings.json',
                      'harness': 'codex', 'request': {'job': 'task', 'item': 'example', 'repo': 'example',
                      'pr': None, 'checkout': 'example', 'head': '0' * 40, 'request_id': 'example'},
                      'argv': ['/opt/shantytown-job-launcher/bin/codex', '--version'],
                      'environment': {'PATH': '/usr/bin:/bin', 'CODEX_HOME': home + '/.codex'}, 'review': None}
        self.new = m.candidate(self.old, m.encoded(self.extra))
        (self.base / (m.NAME + '.json')).write_bytes(self.old)
        (self.base / 'slots.json').write_bytes(self.plan)
        (self.base / 'account-receipts').mkdir()
        self.rp = self.base / 'account-receipts/51001.json'
        self.initial = {'principal': m.NAME, 'uid': 51001, 'sha256': m.OLD, 'state': 'complete'}
        self.rp.write_bytes(m.encoded(self.initial))
        self.rp.chmod(0o600)
        self.addCleanup(patch.stopall)
        patch.object(m, 'JOBS', self.base).start()
        patch.object(m.packet, 'protected').start()  # private dirs replace fixed root-owned native paths
        patch.object(m, 'before_activation').start()
        self.native = patch.object(m, 'native').start()

    def apply(self, mode='expand'):
        m.perform(mode, self.old, self.plan, self.new, 'a' * 40, 'operator')

    def test_identity_bytes_retained_and_exact_eight_additions(self):
        self.assertTrue(self.new.startswith(self.old[:self.old.rfind(b'}')].rstrip() + b','))
        row = json.loads(self.new)
        self.assertEqual({k: row[k] for k in m.IDENTITY}, json.loads(self.old))
        self.assertEqual(set(row) - m.IDENTITY, m.LAUNCH)
        changed = dict(self.extra, uid=51002)
        with self.assertRaises(ValueError):
            m.candidate(self.old, m.encoded(changed))
        with self.assertRaises(ValueError):
            m.candidate(self.old.replace(b'51001', b'51002'), m.encoded(self.extra))

    def test_private_fields_and_environment_refuse(self):
        for change in ({'environment': {'SEEDS_TOKEN': 'not-a-real-token'}}, {'workspace': '/tmp/shared'},
                       {'settings': self.extra['settings'] + '/../escape'}, {'argv': ['relative']},
                       {'review': {}}, {'controller_root': json.loads(self.old)['root']}):
            with self.subTest(change=change), self.assertRaises(ValueError):
                m.candidate(self.old, m.encoded(dict(self.extra, **change)))
        with self.assertRaises(ValueError):
            m.document(b'{"uid":51001,"uid":51002}')

    def test_expand_then_rollback_preserves_history_and_archives(self):
        initial_bytes = self.rp.read_bytes()
        self.apply()
        self.assertTrue(self.rp.read_bytes().startswith(initial_bytes[:initial_bytes.rfind(b'}')].rstrip() + b','))
        expanded_bytes = self.rp.read_bytes()
        expanded = json.loads(expanded_bytes)
        self.assertEqual({k: expanded[k] for k in self.initial}, self.initial)
        self.assertEqual([e['action'] for e in expanded['migrations']], ['expand-prepared', 'expand-complete'])
        archive = self.base / 'migrations/archive' / (m.NAME + '.e985f7f8.json')
        self.assertEqual(archive.read_bytes(), self.old)
        self.assertEqual(archive.stat().st_mode & 0o777, 0o444)
        self.assertEqual(list(self.base.glob('eph-*.json')), [self.base / (m.NAME + '.json')])
        self.apply('rollback')
        self.assertTrue(self.rp.read_bytes().startswith(expanded_bytes[:expanded_bytes.rfind(b']')].rstrip() + b','))
        result = json.loads(self.rp.read_bytes())
        self.assertEqual(result['migrations'][:2], expanded['migrations'])
        self.assertEqual(result['migrations'][-1]['action'], 'rollback-complete')
        self.assertEqual((self.base / (m.NAME + '.json')).read_bytes(), self.old)
        self.assertEqual((self.base / 'slots.json').read_bytes(), self.plan)
        with self.assertRaises(ValueError):
            self.apply()  # rollback does not recycle or erase the migration history

    def test_any_launch_receipt_blocks_expand_and_rollback(self):
        used = self.base / 'used'
        used.mkdir()
        for value in ({'uid': 51001}, {'uid': 51002, 'principal': m.NAME}, {'uid': '51001'}):
            (used / 'used.json').write_bytes(m.encoded(value))
            with self.assertRaises(ValueError):
                self.apply()
            self.assertEqual(json.loads(self.rp.read_bytes()), self.initial)
        (used / 'used.json').unlink()
        self.apply()
        (used / 'used.json').write_bytes(m.encoded({'uid': 51001}))
        with self.assertRaises(ValueError):
            self.apply('rollback')

    def test_unknown_inventory_or_wrong_current_pin_refuses(self):
        (self.base / 'used').mkdir()
        (self.base / 'used/unknown').write_bytes(b'')
        with self.assertRaises(ValueError):
            self.apply()
        (self.base / 'used/unknown').unlink()
        (self.base / 'slots.json').write_bytes(self.plan + b' ')
        with self.assertRaises(ValueError):
            self.apply()
        self.assertEqual(json.loads(self.rp.read_bytes()), self.initial)

    def test_prepared_receipt_retained_after_interruption_and_retry_refuses(self):
        atomic = m.atomic
        def interrupt(path, previous, raw, mode):
            if path.name == 'slots.json':
                raise OSError('simulated power loss')
            atomic(path, previous, raw, mode)
        with patch.object(m, 'atomic', side_effect=interrupt), self.assertRaises(OSError):
            self.apply()
        receipt = json.loads(self.rp.read_bytes())
        self.assertEqual([e['action'] for e in receipt['migrations']], ['expand-prepared'])
        with self.assertRaises(ValueError):
            self.apply()
        with self.assertRaises(ValueError):
            self.apply('rollback')
        self.assertEqual(json.loads(self.rp.read_bytes()), receipt)

    def test_incomplete_original_receipt_and_native_drift_refuse(self):
        for field, value in (('state', 'prepared'), ('sha256', '0' * 64), ('uid', True)):
            self.rp.write_bytes(m.encoded(dict(self.initial, **{field: value})))
            with self.assertRaises(ValueError):
                self.apply()
        self.rp.write_bytes(m.encoded(self.initial))
        self.native.side_effect = ValueError('native account changed')
        with self.assertRaises(ValueError):
            self.apply()
        self.assertEqual((self.base / (m.NAME + '.json')).read_bytes(), self.old)

    def test_metadata_policy_gate_applies_before_any_receipt_change(self):
        m.before_activation.side_effect = ValueError('metadata already applied')
        with self.assertRaises(ValueError):
            self.apply()
        self.assertEqual(json.loads(self.rp.read_bytes()), self.initial)

    def test_occupied_archive_is_never_adopted(self):
        archive = self.base / 'migrations/archive'
        archive.mkdir(parents=True)
        path = archive / (m.NAME + '.e985f7f8.json')
        path.write_bytes(b'foreign')
        with self.assertRaises(ValueError):
            self.apply()
        self.assertEqual(path.read_bytes(), b'foreign')
        self.assertEqual(json.loads(self.rp.read_bytes()), self.initial)

    def test_two_phase_binding_requires_same_item_refused_row_and_retry_budget(self):
        ledger = self.base / 'ledger.sqlite'
        con = sqlite3.connect(ledger)
        con.execute('CREATE TABLE requests(request_id TEXT,kind TEXT,repo TEXT,state TEXT,work_item TEXT,attempts INTEGER,claimed_until REAL)')
        con.execute('INSERT INTO requests VALUES(?,?,?,?,?,?,?)', ('b' * 64, 'install', 'example', 'refused', 'example', 1, None))
        con.commit()
        def check(item='example', attempts=3):
            return m.binding(m.encoded(self.extra), ledger, m.packet.digest(ledger.read_bytes()),
                             'b' * 64, m.encoded({'id': item}), attempts)
        self.assertEqual(check()['item'], 'example')
        with self.assertRaises(ValueError):
            check(item='another')
        with self.assertRaises(ValueError):
            check(attempts=1)
        for column, value in (('work_item', 'another'), ('repo', 'another'), ('state', 'dispatched'),
                              ('attempts', 3), ('claimed_until', 123.0)):
            original = con.execute('SELECT ' + column + ' FROM requests').fetchone()[0]
            con.execute('UPDATE requests SET ' + column + '=?', (value,))
            con.commit()
            with self.assertRaises(ValueError):
                check()
            con.execute('UPDATE requests SET ' + column + '=?', (original,))
            con.commit()
        con.close()
        Path(str(ledger) + '-wal').touch()
        with self.assertRaises(ValueError):
            check()

    def test_digest_mismatch_refuses(self):
        path = self.base / 'input.json'
        path.write_bytes(b'{}')
        with self.assertRaises(ValueError):
            m.pinned(path, '0' * 64)


if __name__ == '__main__':
    unittest.main()
