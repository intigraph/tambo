"""Exercise the actual optional sync block without running any installer lane."""
import json
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
SCRIPT = (ROOT / "scripts/setup-environment.sh").read_text()
BLOCK = SCRIPT[SCRIPT.index("# Repo pendants are opt-in"):SCRIPT.index("# ── Verify:")]


class PendantSetup(unittest.TestCase):
    def probe(self, *, available=True, authority=True, status=0, requested=True, monorepo=None):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            repo = root / "checkout with spaces"
            if available:
                repo.mkdir()
            binary = root / "caboodle"
            binary.write_text('#!/usr/bin/python3\nimport json,os,sys\n'
                              'open(os.environ["CALLS"],"w").write(json.dumps(sys.argv[1:]))\n'
                              'sys.exit(int(os.environ["STATUS"]))\n')
            binary.chmod(0o755)
            env = {"PATH": str(root) + ":/usr/bin:/bin", "CALLS": str(root / "calls"),
                   "STATUS": str(status), "HOME": str(root)}
            if requested:
                env["CABOODLE_PACKS_REPO"] = str(repo)
            if monorepo is not None:
                env["CABOODLE_PACKS_MONOREPO"] = monorepo
            if authority:
                env["CABOODLE_PACKS_SHAPES_FROM"] = str(root / "authority with spaces.pendant.tar.gz")
            result = subprocess.run(["sh", "-c", "have() { command -v \"$1\" >/dev/null 2>&1; }\n"
                                     + BLOCK + '\nprintf "%s\\n" "$PENDANT_STATUS"\nexit 0\n'],
                                    env=env, capture_output=True, text=True, timeout=10)
            self.assertEqual(result.returncode, 0, result.stderr)
            calls = json.loads((root / "calls").read_text()) if (root / "calls").exists() else None
            return result.stdout, calls, str(repo), env.get("CABOODLE_PACKS_SHAPES_FROM")

    def test_available_checkout_uses_the_same_sync_command(self):
        output, calls, repo, authority = self.probe()
        self.assertEqual(calls, ["packs", "sync", "--repo", repo, "--shapes-from", authority])
        self.assertIn("sync completed", output)

    def test_precheckout_absence_and_missing_authority_do_not_invoke_sync(self):
        for kwargs, message in [({"available": False}, "not available yet"),
                                ({"authority": False}, "must name the trusted"),
                                ({"requested": False}, "not requested")]:
            with self.subTest(kwargs=kwargs):
                output, calls, _, _ = self.probe(**kwargs)
                self.assertIsNone(calls)
                self.assertIn(message, output)

    def test_refusal_is_reported_while_optional_setup_still_exits_zero(self):
        output, calls, _, _ = self.probe(status=3)
        self.assertIsNotNone(calls)
        self.assertIn("FAILED: repository pendant sync refused", output)

    def test_monorepo_is_explicit_and_invalid_mode_does_not_run_sync(self):
        output, calls, repo, authority = self.probe(monorepo="1")
        self.assertEqual(calls, ["packs", "sync", "--repo", repo, "--shapes-from", authority, "--monorepo"])
        self.assertIn("sync completed", output)
        output, calls, _, _ = self.probe(monorepo="unexpected")
        self.assertIsNone(calls)
        self.assertIn("FAILED", output)


if __name__ == "__main__":
    unittest.main()
