#!/usr/bin/env python3
"""Pin the release pendant admission gate (aegis-dics, seeds/desire-path shape).

# arming: ci; release.yml's pendant jobs only run on a release, so CI pins them.
"""
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
GATE = "vars.TAMBO_PENDANT_PUBLICATION == 'enabled'"


def job(name):
    """The text of one top-level job in release.yml (no YAML dependency in CI)."""
    text = (ROOT / ".github/workflows/release.yml").read_text()
    start = text.index("\n  " + name + ":\n") + 1
    rest = text[start:].split("\n")
    lines = [rest[0]]
    for row in rest[1:]:
        if row.startswith("  ") and not row.startswith("   ") and row.strip():
            break
        lines.append(row)
    return "\n".join(lines)


def condition(name):
    return next(row.split("if:", 1)[1].strip() for row in job(name).splitlines()
                if row.startswith("    if:"))


def record(admission):
    """Run the recorded-state step with ADMISSION set; return (stdout, summary)."""
    block = job("repository-pendant-admission").split("        run: |\n", 1)[1]
    step = "\n".join(row[10:] for row in block.splitlines() if row.startswith("          "))
    with tempfile.TemporaryDirectory() as temp:
        summary = Path(temp) / "summary"
        result = subprocess.run(["bash", "-euc", step], capture_output=True, text=True, timeout=10,
                                env={"ADMISSION": admission, "GITHUB_STEP_SUMMARY": str(summary),
                                     "PATH": "/usr/bin:/bin"})
        if result.returncode:
            raise AssertionError(result.stderr)
        return result.stdout, summary.read_text()


class PendantAdmission(unittest.TestCase):
    def test_verify_runs_only_when_admitted(self):
        self.assertIn(GATE, condition("repository-pendant"))
        self.assertIn("needs.publish.result == 'success'", condition("repository-pendant"))
        # Admitted, the verify stays strict: it still runs the bounded verifier.
        self.assertIn("scripts/verify-repository-share.py", job("repository-pendant"))

    def test_not_admitted_is_recorded_not_silent(self):
        self.assertNotIn("vars.", condition("repository-pendant-admission"))  # every publish
        for value in ("", "disabled", "true"):
            with self.subTest(admission=value):
                out, summary = record(value)
                self.assertIn("NOT ADMITTED: pendant publication not admitted for tambo", out)
                self.assertIn("not a pass", summary)

    def test_enabled_is_recorded_as_admitted(self):
        out, summary = record("enabled")
        self.assertIn("admitted: the trusted host publisher must deliver", out)
        self.assertNotIn("NOT ADMITTED", summary)


if __name__ == "__main__":
    unittest.main()
