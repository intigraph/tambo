#!/usr/bin/env python3
"""Offline controls for the public stack-table drift check."""

import importlib.util
from contextlib import redirect_stderr, redirect_stdout
import io
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch, MagicMock

SCRIPT = Path(__file__).with_name("check-stack-table.py")
CANONICAL = SCRIPT.parents[1] / "docs/stack-table.md"
SPEC = importlib.util.spec_from_file_location("stack_table", SCRIPT)
CHECK = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(CHECK)


class StackTableControls(unittest.TestCase):
    def check_copy(self, data):
        with tempfile.TemporaryDirectory() as directory:
            readme = Path(directory) / "README.md"
            readme.write_bytes(data)
            return subprocess.run(
                [sys.executable, str(SCRIPT), "--canonical", str(CANONICAL), str(readme)],
                capture_output=True, text=True, check=False,
            )

    def test_canonical_copy_passes_but_changed_wording_fails(self):
        data = CANONICAL.read_bytes()
        self.assertEqual(self.check_copy(data).returncode, 0)
        changed = data.replace(b"can read policies from Quipu", b"ignores policies from Quipu")
        self.assertNotEqual(changed, data)
        result = self.check_copy(changed)
        self.assertEqual(result.returncode, 1)
        self.assertIn("differs", result.stderr)

    def test_lead_drift_is_not_normalized_away(self):
        data = CANONICAL.read_bytes()
        changed = data.replace(b"governed, bitemporal", b"unaccountable, timeless")
        self.assertNotEqual(changed, data)
        self.assertEqual(self.check_copy(changed).returncode, 1)

    def test_lead_without_table_or_separator_fails(self):
        for body in (b"An overview.\n", b"An overview.\n\n| tool | value |\n| bad | row |\n"):
            with self.subTest(body=body):
                with self.assertRaisesRegex(ValueError, "complete stack overview"):
                    CHECK.marked_table(CHECK.START + b"\n" + body + CHECK.END + b"\n", "fixture")

    def test_missing_duplicate_or_reversed_markers_fail(self):
        data = CANONICAL.read_bytes()
        for bad in (data.replace(CHECK.START, b""), data + CHECK.START,
                    CHECK.END + b"\n" + CHECK.START + b"\n"):
            with self.subTest(data=bad[:50]):
                self.assertEqual(self.check_copy(bad).returncode, 1)

    def test_whitespace_drift_is_not_normalized_away(self):
        data = CANONICAL.read_bytes().replace(b"| tool |", b"|  tool |")
        self.assertEqual(self.check_copy(data).returncode, 1)

    def test_remote_error_is_not_a_clean_zero(self):
        with patch.object(CHECK, "remote_readme", side_effect=OSError("offline")), \
             patch.object(sys, "argv", [str(SCRIPT), "--remote"]), \
             redirect_stdout(io.StringIO()), redirect_stderr(io.StringIO()):
            self.assertEqual(CHECK.main(), 1)

    def test_shantytown_is_audited_and_its_missing_block_fails(self):
        canonical = CANONICAL.read_bytes()
        for missing, expected in ((False, 0), (True, 1)):
            seen = []

            def read(repo):
                seen.append(repo)
                return b"A README without a stack block.\n" if missing and repo == "shantytown" else canonical

            with self.subTest(missing=missing), \
                 patch.object(CHECK, "remote_readme", side_effect=read), \
                 patch.object(sys, "argv", [str(SCRIPT), "--remote"]), \
                 redirect_stdout(io.StringIO()), redirect_stderr(io.StringIO()):
                self.assertEqual(CHECK.main(), expected)
                self.assertEqual(seen.count("shantytown"), 1)
                self.assertEqual(len(seen), 8)

    def test_remote_response_has_a_size_limit(self):
        response = MagicMock()
        response.__enter__.return_value.read.return_value = b"x" * (CHECK.MAX_BYTES + 1)
        with patch.object(CHECK, "urlopen", return_value=response):
            with self.assertRaisesRegex(ValueError, "size limit"):
                CHECK.remote_readme("caboodle")


if __name__ == "__main__":
    unittest.main()
