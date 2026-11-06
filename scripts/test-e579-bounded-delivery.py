#!/usr/bin/env python3
"""Real CLI controls with a disposable fake receiver, never a native/live job."""
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

SCRIPT = Path(__file__).with_name('e579-bounded-delivery.py').resolve()


class BudgetTests(unittest.TestCase):
    def fixture(self, root):
        receiver = root / 'fake_receiver.py'
        receiver.write_text('import pathlib,sys\np=pathlib.Path(__file__).with_name("calls")\np.write_text(p.read_text()+"1" if p.exists() else "1")\nsys.exit(75)\n')
        routing = root / 'routing.yaml'; routing.write_text('fixture: true\n')
        event = root / 'event.json'; event.write_text('{"fixture": true}')
        pin = lambda path: hashlib.sha256(path.read_bytes()).hexdigest()
        job = dict(version=1, max_attempts=2, receiver_script=str(receiver), receiver_sha256=pin(receiver),
                   routing_config=str(routing), routing_sha256=pin(routing), event=str(event),
                   event_sha256=pin(event), state=str(root / 'state'))
        path = root / 'job.json'; path.write_text(json.dumps(job))
        return path, job

    def run_job(self, path):
        return subprocess.run([sys.executable, '-B', str(SCRIPT), '--job-config', str(path)],
                              capture_output=True, text=True)

    def test_two_explicit_attempts_then_third_refuses_without_child(self):
        with tempfile.TemporaryDirectory() as d:
            root = Path(d); path, job = self.fixture(root)
            for attempt in (1, 2):
                result = self.run_job(path)
                self.assertEqual(result.returncode, 75, result.stderr)
                self.assertIn('reserved attempt=' + str(attempt), result.stdout)
                self.assertEqual((root / 'calls').read_text(), '1' * attempt)
            result = self.run_job(path)
            self.assertEqual(result.returncode, 1)
            self.assertIn('budget exhausted', result.stderr)
            self.assertEqual((root / 'calls').read_text(), '11')
            receipt = json.loads((root / 'state/attempts.json').read_text())
            self.assertEqual(receipt['attempts'], 2)

    def test_config_or_source_change_cannot_reset_budget(self):
        with tempfile.TemporaryDirectory() as d:
            root = Path(d); path, job = self.fixture(root)
            self.assertEqual(self.run_job(path).returncode, 75)
            path.write_text(json.dumps(job, indent=2))
            result = self.run_job(path)
            self.assertIn('reservation changed', result.stderr)
            self.assertEqual((root / 'calls').read_text(), '1')
            Path(job['receiver_script']).write_text('raise Exception("not admitted")')
            self.assertIn('pinned input changed', self.run_job(path).stderr)
            self.assertEqual((root / 'calls').read_text(), '1')

    def test_other_budget_or_symlink_state_refuses_before_child(self):
        with tempfile.TemporaryDirectory() as d:
            root = Path(d); path, job = self.fixture(root)
            for value in (True, 0, 1, 3, '2'):
                changed = dict(job, max_attempts=value); path.write_text(json.dumps(changed))
                self.assertNotEqual(self.run_job(path).returncode, 0)
                self.assertFalse((root / 'calls').exists())
            path.write_text(json.dumps(job))
            target = root / 'target'; target.mkdir(mode=0o700)
            (root / 'state').symlink_to(target, target_is_directory=True)
            self.assertIn('state directory', self.run_job(path).stderr)
            self.assertFalse((root / 'calls').exists())

    def test_interrupted_reservation_is_retained_and_refuses(self):
        with tempfile.TemporaryDirectory() as d:
            root = Path(d); path, job = self.fixture(root)
            state = root / 'state'; state.mkdir(mode=0o700)
            lock = state / 'lock'; lock.write_bytes(b'reserved\n'); lock.chmod(0o600)
            result = self.run_job(path)
            self.assertIn('interrupted reservation', result.stderr)
            self.assertFalse((root / 'calls').exists())
            self.assertEqual(lock.read_bytes(), b'reserved\n')


if __name__ == '__main__': unittest.main()
