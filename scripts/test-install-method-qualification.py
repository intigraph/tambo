#!/usr/bin/env python3
"""Offline boundary controls; these never invoke a package manager or Docker."""
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

SPEC = importlib.util.spec_from_file_location("qualification", Path(__file__).with_name("qualify-install-method.py"))
Q = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(Q)


class QualificationTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.env = dict(GITHUB_ACTIONS="true", INSTALL_METHOD_RUNNER="github-hosted",
                        RUNNER_OS="macOS", GITHUB_REPOSITORY="intigraph/tambo",
                        RUNNER_TEMP=str(self.root), GITHUB_SHA="a" * 40, GITHUB_RUN_ID="123")

    def test_live_local_self_hosted_wrong_repository_and_wrong_os_are_refused(self):
        Q.require_hosted("brew", self.root / "new", self.env)
        for key, value in [("GITHUB_ACTIONS", "false"), ("INSTALL_METHOD_RUNNER", "self-hosted"),
                           ("GITHUB_REPOSITORY", "other/tambo"), ("RUNNER_OS", "Linux")]:
            env = dict(self.env); env[key] = value
            with self.assertRaises(Q.Failure):
                Q.require_hosted("brew", self.root / "new", env)
        with patch.dict(os.environ, {}, clear=True):
            self.assertEqual(Q.main(["brew", "--output", str(self.root / "refused")]), 2)
        self.assertFalse((self.root / "refused").exists())

    def test_output_cannot_reuse_old_proof_or_escape_the_runner_temp(self):
        old = self.root / "old"; old.mkdir()
        for path in [old, self.root, self.root.parent / "escape"]:
            with self.assertRaises(Q.Failure):
                Q.require_hosted("brew", path, self.env)
        link = self.root / "link"; link.symlink_to(self.root.parent, target_is_directory=True)
        with self.assertRaises(Q.Failure):
            Q.require_hosted("brew", link / "escape", self.env)

    def test_image_input_is_a_fixed_public_version_or_digest_not_a_shell_fragment(self):
        for value in ["ghcr.io/intigraph/quipu:0.8.1", "ghcr.io/intigraph/quipu@sha256:" + "a" * 64]:
            self.assertEqual(Q.image_reference(value), value)
        for value in ["--privileged", "ghcr.io/intigraph/quipu:latest", "other/quipu:0.1",
                      "ghcr.io/intigraph/quipu:0.1;id", "https://example.test/image"]:
            with self.assertRaises(Q.Failure):
                Q.image_reference(value)

    def test_signing_identity_checks_primary_keys_not_matching_subkeys(self):
        def fpr(value):
            return "fpr:::::::::" + value + ":\n"
        good = "pub::::::::::\n" + fpr(Q.FINGERPRINT) + "sub::::::::::\n" + fpr("B" * 40)
        self.assertEqual(Q.primary_fingerprints(good), [Q.FINGERPRINT])
        wrong = "pub::::::::::\n" + fpr("B" * 40) + "sub::::::::::\n" + fpr(Q.FINGERPRINT)
        self.assertNotEqual(Q.primary_fingerprints(wrong), [Q.FINGERPRINT])
        self.assertNotEqual(Q.primary_fingerprints(good + "pub::::::::::\n" + fpr("C" * 40)), [Q.FINGERPRINT])

    def test_mcp_control_requires_both_successful_actual_responses(self):
        a = dict(jsonrpc="2.0", id=1, result={"protocolVersion": "2024-11-05"})
        b = dict(jsonrpc="2.0", id=2, result={"tools": [{"name": "read"}]})
        render = lambda rows: "\n".join(json.dumps(row) for row in rows)
        self.assertEqual(set(Q.mcp_responses(render([a, b]))), {1, 2})
        for rows in [[a], [b], [a, a, b], [a, dict(id=2, error={"message": "refused"})],
                     [a, dict(id=2, result={"tools": []})], [dict(a, id=True), b]]:
            with self.assertRaises(Q.Failure):
                Q.mcp_responses(render(rows))

    def test_versions_require_actual_consistent_banners_and_ignore_sha_line_changes(self):
        def run(outputs):
            source = iter(outputs)
            return type("Fixture", (), {"command": lambda _, argv: subprocess.CompletedProcess(argv, 0, next(source), "")})()
        _, value = Q.versions(run(["quipu 0.17.0\ngit_sha: one", "quipu-server 0.17.0\ngit_sha: two"]), ["cli", "server"])
        self.assertEqual(value, "0.17.0")
        for banners in [["not quipu", "quipu 0.17.0"], ["quipu 0.16.0", "quipu-server 0.17.0"]]:
            with self.assertRaises(Q.Failure):
                Q.versions(run(banners), ["cli", "server"])

    def test_cleanup_runs_in_reverse_even_when_an_earlier_cleanup_fails(self):
        with patch.dict(os.environ, self.env, clear=True):
            run = Q.Run("container", self.root / "receipt")
        calls = []
        def bad():
            calls.append("container")
            raise Q.Failure("failed cleanup")
        run.cleanup = [lambda: calls.append("image"), lambda: calls.append("volume"), bad]
        self.assertEqual(run.finish(Q.Failure("original failure")), 1)
        self.assertEqual(calls, ["container", "volume", "image"])
        report = json.loads((run.root / "result.json").read_text())
        self.assertEqual(report["status"], "FAIL")
        self.assertTrue(report["cleanup_errors"])

    def test_timeout_retains_partial_output_and_is_not_a_success(self):
        with patch.dict(os.environ, self.env, clear=True):
            run = Q.Run("container", self.root / "receipt")
        with self.assertRaisesRegex(Q.Failure, "timed out"):
            run.command([sys.executable, "-c", "import time; print('before timeout', flush=True); time.sleep(10)"],
                        timeout=0.2)
        self.assertTrue(run.report["commands"][0]["timed_out"])
        self.assertIsNone(run.report["commands"][0]["exit"])
        self.assertIn("before timeout", (run.root / "00.stdout.log").read_text())

    def brew_fixture(self, *, preinstalled=False, partial_failure=False, delete_data=False):
        prefix = self.root / "prefix"; (prefix / "bin").mkdir(parents=True)
        state = dict(installed=preinstalled)
        calls = []
        def command(run, argv, **kwargs):
            calls.append(argv)
            stdout = ""
            if argv == ["brew", "--prefix"]:
                stdout = str(prefix)
            elif argv == ["brew", "list", "--formula"]:
                stdout = "quipu\n" if state["installed"] else ""
            elif argv[:2] == ["brew", "--repository"]:
                stdout = str(self.root / "tap")
            elif argv[:2] == ["brew", "install"]:
                state["installed"] = True
                for name in ["quipu", "quipu-server"]:
                    (prefix / "bin" / name).write_text("fake owned executable")
                (prefix / "bin/qp").symlink_to("quipu")
                if partial_failure:
                    raise Q.Failure("injected install failure after partial ownership")
            elif argv[:2] == ["brew", "uninstall"]:
                state["installed"] = False
                for name in ["quipu", "quipu-server", "qp"]:
                    (prefix / "bin" / name).unlink()
                if delete_data:
                    run.data.unlink()
            elif argv[-1] == "--version":
                stdout = "quipu 0.17.0\n"
            elif argv[:1] == ["git"]:
                stdout = "b" * 40
            return subprocess.CompletedProcess(argv, 0, stdout, "")
        output = self.root / "receipt"
        with patch.dict(os.environ, self.env, clear=True), patch.object(Q.Run, "command", command):
            result = Q.main(["brew", "--output", str(output)])
        return result, state, calls, json.loads((output / "result.json").read_text())

    def test_clean_brew_path_removes_owned_commands_preserves_data_and_reports_noop_upgrade(self):
        code, state, calls, report = self.brew_fixture()
        self.assertEqual(code, 0)
        self.assertFalse(state["installed"])
        self.assertTrue(report["baseline_absent"])
        self.assertTrue(report["package_and_alias_removed"])
        self.assertTrue(report["data_preserved_by_package_removal"])
        self.assertTrue(report["explicit_owned_data_removal"])
        self.assertFalse(report["upgrade_version_changed"])
        self.assertIn(["brew", "upgrade", "quipu"], calls)

    def test_live_or_preinstalled_formula_never_gets_uninstalled(self):
        code, state, calls, report = self.brew_fixture(preinstalled=True)
        self.assertEqual(code, 1)
        self.assertTrue(state["installed"])
        self.assertFalse(any(row[:2] == ["brew", "uninstall"] for row in calls))

    def test_partial_failed_install_is_not_a_pass_and_still_cleans_only_its_ownership(self):
        code, state, calls, report = self.brew_fixture(partial_failure=True)
        self.assertEqual(code, 1)
        self.assertFalse(state["installed"])
        self.assertEqual(report["status"], "FAIL")
        self.assertIn(["brew", "uninstall", "quipu"], calls)

    def test_an_uninstaller_that_erases_data_makes_the_qualification_fail(self):
        code, _, _, report = self.brew_fixture(delete_data=True)
        self.assertEqual(code, 1)
        self.assertEqual(report["status"], "FAIL")
        self.assertIn("data marker", report["error"])


if __name__ == "__main__":
    unittest.main()
