#!/usr/bin/env python3
# arming: ci Documentation workflow, after its pinned lychee install
"""Real-lychee controls for the production self-repository remap wrapper."""
import json
import os
from pathlib import Path
import shutil
import socket
import subprocess
import tempfile
import threading
import unittest
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

REPO = Path(__file__).resolve().parent.parent
OWN = "https://github.com/scbrown/caboodle/blob/main/"
TARGETS = [
    "skills/caboodle/SKILL.md",
    "docs/design/txtai-gap-analysis.md",
    "docs/design/vision.md",
    "docs/stack/README-STANDARD.md",
    "docs/stack/README-TEMPLATE.md",
]


class DocumentationLinks(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="caboodle-doc-links-")
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name) / "space $cash #hash"
        (self.root / "scripts").mkdir(parents=True)
        shutil.copyfile(REPO / "scripts/check-doc-links.sh", self.root / "scripts/check-doc-links.sh")
        shutil.copyfile(REPO / ".lychee.toml", self.root / ".lychee.toml")
        self.env = dict(os.environ, HOME=str(self.root))
        for name in ("GITHUB_TOKEN", "GH_TOKEN"):
            self.env.pop(name, None)
        self.input = self.root / "input.md"

    def check(self, links, offline=True):
        self.input.write_text("\n".join(f"[target]({url})" for url in links) + "\n")
        argv = [
            "sh", str(self.root / "scripts/check-doc-links.sh"),
            "--no-progress", "--format", "json", "--max-retries", "0", "--timeout", "2",
        ]
        if offline:
            argv.append("--offline")
        site = self.root / "site"
        site.mkdir(exist_ok=True)
        argv += ["--root-dir", str(site), str(self.input)]
        result = subprocess.run(argv, env=self.env, capture_output=True, text=True, timeout=20)
        self.assertTrue(result.stdout.startswith("{"), result.stdout + result.stderr)
        return result.returncode, json.loads(result.stdout)

    def test_actual_five_targets_checked_not_excluded_and_deleted_target_fails(self):
        for target in TARGETS:
            dest = self.root / target
            dest.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(REPO / target, dest)
        links = [OWN + target for target in TARGETS]
        rc, result = self.check(links)
        self.assertEqual(rc, 0, result)
        self.assertEqual((result["successful"], result["remaps"], result["excludes"]), (5, 5, 0))
        (self.root / TARGETS[0]).unlink()
        rc, result = self.check(links)
        self.assertNotEqual(rc, 0)
        self.assertEqual((result["successful"], result["errors"], result["excludes"]), (4, 1, 0))
        self.assertIn("SKILL.md", json.dumps(result["error_map"]))

    def test_existing_anchor_passes_missing_anchor_fails(self):
        (self.root / "target.md").write_text("# Target\n\n## Present\n")
        rc, result = self.check([OWN + "target.md#present"])
        self.assertEqual(rc, 0, result)
        self.assertEqual((result["successful"], result["excludes"]), (1, 0))
        rc, result = self.check([OWN + "target.md#missing"])
        self.assertNotEqual(rc, 0)
        self.assertEqual(result["errors"], 1)

    def test_other_owner_repo_and_branch_are_not_remapped(self):
        # Offline is a scope probe, not evidence that remote links pass.
        # An overbroad remap would turn these missing paths into file errors.
        links = [
            "https://github.com/other/caboodle/blob/main/missing.md",
            "https://github.com/scbrown/other/blob/main/missing.md",
            "https://github.com/scbrown/caboodle/blob/v1.0.0/missing.md",
            "https://github.com/scbrown/caboodle/tree/main/missing.md",
        ]
        rc, result = self.check(links)
        self.assertEqual(rc, 0, result)
        self.assertEqual((result["remaps"], result["excludes"], result["successful"]), (0, 4, 0))

    def test_normal_mode_still_checks_unrelated_remote_failure(self):
        # A real disposable HTTP server supplies a positive 200 control and a
        # negative 404. NORMAL mode must contact it; no offline switch may
        # swallow the error. IPv6 is outside the illustrative URL exclusions.
        hits = []

        class Handler(BaseHTTPRequestHandler):
            def do_GET(self):
                hits.append(self.path)
                self.send_response(200 if self.path == "/present" else 404)
                self.end_headers()
                self.wfile.write(b"fixture")

            def log_message(self, *args):
                pass

        class Server(ThreadingHTTPServer):
            address_family = socket.AF_INET6

        server = Server(("::1", 0), Handler)
        thread = threading.Thread(target=server.serve_forever, daemon=True)
        thread.start()
        self.addCleanup(server.server_close)
        self.addCleanup(server.shutdown)
        base = f"http://[::1]:{server.server_port}"
        rc, result = self.check([base + "/present"], offline=False)
        self.assertEqual(rc, 0, result)
        self.assertEqual((result["successful"], result["excludes"]), (1, 0))
        rc, result = self.check([base + "/missing"], offline=False)
        self.assertNotEqual(rc, 0)
        self.assertEqual((result["errors"], result["excludes"], result["remaps"]), (1, 0, 0))
        self.assertIn("/missing", hits)
        self.assertIn("404", json.dumps(result["error_map"]))


if __name__ == "__main__":
    unittest.main(verbosity=2)
