#!/usr/bin/env python3
"""Exercise the real inventory against new branding leaks and retained contracts."""
import importlib.util
import json
from pathlib import Path
import subprocess
import tempfile
import unittest

spec = importlib.util.spec_from_file_location('product_name', Path(__file__).with_name('check-product-name.py'))
checker = importlib.util.module_from_spec(spec)
spec.loader.exec_module(checker)


class ProductName(unittest.TestCase):
    def test_exact_contract_and_new_current_or_untracked_reference(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            subprocess.run(['git', 'init', '-q', root], check=True)
            (root / 'state.md').write_text('Keep .caboodle/state.json\n')
            ledger = root / checker.LEDGER
            ledger.parent.mkdir(parents=True)
            ledger.write_text(json.dumps({'version': 1, 'files': {
                name: {'reason': 'Existing state path', 'lines': lines}
                for name, lines in checker.inventory(root).items()
            }}))
            self.assertEqual(checker.check(root, ledger)[0], [])
            (root / 'README.md').write_text('# Caboodle\n')
            self.assertEqual(checker.check(root, ledger)[0], ['README.md'])
            (root / 'README.md').write_text('# Tambo\n')
            self.assertEqual(checker.check(root, ledger)[0], [])
            (root / 'state.md').write_text('Keep .caboodle/other.json\n')
            self.assertEqual(checker.check(root, ledger)[0], ['state.md'])

    def test_reason_is_required(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            subprocess.run(['git', 'init', '-q', root], check=True)
            ledger = root / 'ledger.json'
            ledger.write_text(json.dumps({'version': 1, 'files': {'fixture': {'lines': {}}}}))
            with self.assertRaises(ValueError):
                checker.check(root, ledger)


if __name__ == '__main__':
    unittest.main()
