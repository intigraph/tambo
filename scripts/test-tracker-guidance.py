#!/usr/bin/env python3
"""Controls for check-tracker-guidance.py (aegis-bqcq)."""
import importlib.util
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / "scripts/check-tracker-guidance.py"
spec = importlib.util.spec_from_file_location("tracker", SCRIPT)
tracker = importlib.util.module_from_spec(spec)
spec.loader.exec_module(tracker)


def kinds(text):
    return [kind for _, kind, _ in tracker.findings(text)]


class Findings(unittest.TestCase):
    def test_command_forms_refused(self):
        for line in ("Run `br ready` to pick work.", "bd create \"title\"",
                     "    br comments add x", "then `bd close aegis-1`"):
            with self.subTest(line=line):
                self.assertEqual(kinds(line), ["br/bd command"])

    def test_tool_named_as_tracker_refused(self):
        # The exact row Tambo's corpus page carried before this change.
        self.assertEqual(kinds("| beads (`br`) | issue tracking as agent memory — SQLite + JSONL |"),
                         ["br/bd presented as the tracker"])

    def test_seeds_and_legacy_facts_allowed(self):
        for line in ("Track work with Seeds: `sd ready` lists unblocked items.",
                     "`tambo queue-br export.jsonl` imports a legacy export",
                     "Path command alias `bd -> sd`, so an agent that types `bd` from habit",
                     "the shared tooling (`just`, `bd`, `pre-commit`)",
                     "a branch named feature/br-update or a file sub.bd create"):
            with self.subTest(line=line):
                self.assertEqual(kinds(line), [])

    def test_marker_exempts_only_its_line(self):
        text = ("| bead, `br` | An issue in the older beads tracker. <!-- legacy-tracker --> |\n"
                "| next | Run `br ready`. |")
        self.assertEqual([n for n, _, _ in tracker.findings(text)], [2])


class Command(unittest.TestCase):
    def run_on(self, files):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            for name, body in files.items():
                (root / name).parent.mkdir(parents=True, exist_ok=True)
                (root / name).write_text(body)
            return subprocess.run([sys.executable, str(SCRIPT), str(root)],
                                  capture_output=True, text=True, timeout=30)

    def test_clean_tree_passes_and_new_instruction_fails(self):
        clean = self.run_on({"docs/book/src/a.md": "Use `sd show <id>`.\n"})
        self.assertEqual(clean.returncode, 0, clean.stderr)
        dirty = self.run_on({"docs/book/src/a.md": "Use `sd show <id>`.\n",
                             "skills/x/SKILL.md": "Next: `br update x --status closed`\n"})
        self.assertEqual(dirty.returncode, 1, dirty.stdout)
        self.assertIn("skills/x/SKILL.md:1: br/bd command", dirty.stdout)

    def test_missing_book_is_unknown_not_clean(self):
        empty = self.run_on({"README.md": "nothing\n"})
        self.assertEqual(empty.returncode, 2, empty.stdout)

    def test_vision_uses_the_same_tracker_contract(self):
        dirty = self.run_on({"docs/book/src/a.md": "Use `sd show <id>`.\n",
                             "docs/design/vision.md": "The tracker (`br`) installs.\n"})
        self.assertEqual(dirty.returncode, 1, dirty.stdout)
        self.assertIn("docs/design/vision.md:1:", dirty.stdout)

    def test_this_repository_is_clean(self):
        result = subprocess.run([sys.executable, str(SCRIPT), str(ROOT)],
                                capture_output=True, text=True, timeout=30)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)


if __name__ == "__main__":
    unittest.main()
