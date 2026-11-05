#!/usr/bin/env python3
"""Tests for scripts/memory-redirect-hook.py (aegis-gshu.1).

Every file touched is a scratch copy: no test reads or writes ~/.claude or
~/.codex. A stub `cy` stands in for camayoc's hook so the suite runs without
camayoc installed. Set CAMAYOC_SRC to a camayoc checkout to also install and
verify the real hook from that checkout.
"""
from __future__ import annotations

import importlib.util
import io
import json
import os
import stat
import sys
import tempfile
import textwrap
import tomllib
import unittest
from contextlib import redirect_stderr, redirect_stdout
from pathlib import Path

HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location("memory_redirect_hook", HERE / "memory-redirect-hook.py")
mrh = importlib.util.module_from_spec(spec)
spec.loader.exec_module(mrh)

STUB = textwrap.dedent("""\
    #!/usr/bin/env python3
    # A stand-in for camayoc's `cy hook memory-redirect`: same flags, same log, same exits.
    import json, os, subprocess, sys, time
    args = sys.argv[1:]
    if args[:2] != ["hook", "memory-redirect"]:
        sys.exit(2)
    if "--help" in args:
        print("usage: cy hook memory-redirect [--mode {off,copy,block}] [--person IRI]")
        sys.exit(0)
    mode = os.environ.get("CAMAYOC_MEMORY_REDIRECT") or args[args.index("--mode") + 1]
    if mode == "BROKEN":
        sys.exit(0)
    payload = json.load(sys.stdin)
    state = os.path.join(os.environ["XDG_STATE_HOME"], "camayoc")
    os.makedirs(state, exist_ok=True)
    log = os.path.join(state, "memory-redirect.jsonl")
    nonce = os.environ.get("CAMAYOC_REDIRECT_VERIFY")
    def write(event, **kw):
        with open(log, "a") as fh:
            fh.write(json.dumps({"ts": "2026-10-28T00:00:00Z", "event": event, "verify": nonce, **kw}) + "\\n")
    write("fired", mode=mode, tool=payload.get("tool_name"))
    if mode == "block":
        print("Not written: memory files are retired here; record it in Quipu instead:", file=sys.stderr)
        print("  cy record directive --rule 'x' --topic tambo-verify", file=sys.stderr)
        sys.exit(2)
    if mode == "copy" and os.environ.get("STUB_NO_WORKER") != "1":
        line = json.dumps({"ts": "x", "event": "verified", "entity": "directive", "code": 0, "verify": nonce})
        subprocess.Popen(["sh", "-c", f"sleep 0.3; printf '%s\\\\n' '{line}' >> '{log}'"],
                         start_new_session=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    sys.exit(0)
    """)


def run(*argv: str) -> tuple[int, str, str]:
    out, err = io.StringIO(), io.StringIO()
    with redirect_stdout(out), redirect_stderr(err):
        code = mrh.run(list(argv))
    return code, out.getvalue(), err.getvalue()


class Base(unittest.TestCase):

    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.root = Path(self.tmp.name)
        self.cy = self.root / "bin/cy"
        self.cy.parent.mkdir()
        self.cy.write_text(STUB)
        self.cy.chmod(self.cy.stat().st_mode | stat.S_IXUSR)
        self.claude = self.root / "claude/settings.json"
        self.codex = self.root / "codex/config.toml"
        self.common = ["--cy", str(self.cy), "--claude-settings", str(self.claude), "--codex-config", str(self.codex)]
        for k in ("CAMAYOC_MEMORY_REDIRECT", "STUB_NO_WORKER", "CAMAYOC_PERSON", "CLAUDE_CODE_OAUTH_TOKEN",
                  "ANTHROPIC_API_KEY"):
            os.environ.pop(k, None)
        os.environ["XDG_CONFIG_HOME"] = str(self.root / "xdg-config")
        os.environ["CAMAYOC_PERSON"] = "person-test"

    def tearDown(self):
        for k in ("XDG_CONFIG_HOME", "CAMAYOC_PERSON"):
            os.environ.pop(k, None)
        self.tmp.cleanup()

    def command(self) -> str:
        return mrh.claude_entries(self.claude)[0]["command"]


class InstallTests(Base):

    def test_person_comes_from_flag_env_or_camayoc_config_and_is_required(self):
        os.environ.pop("CAMAYOC_PERSON")
        code, _, err = run("install", "--harness", "claude", *self.common)
        self.assertEqual(2, code)
        self.assertIn("--no-person", err)
        self.assertFalse(self.claude.exists(), "a refused install writes nothing")
        code, out, _ = run("install", "--harness", "claude", "--no-person", *self.common)
        self.assertEqual(0, code)
        self.assertIn("warning: installed without --person", out)
        self.assertNotIn("--person", self.command())
        cfg = self.root / "xdg-config/camayoc/config.toml"
        cfg.parent.mkdir(parents=True)
        cfg.write_text('[memory]\nperson = "person-from-config"\n')
        run("install", "--harness", "claude", *self.common)
        self.assertIn("--person person-from-config", self.command())
        os.environ["CAMAYOC_PERSON"] = "person-from-env"
        run("install", "--harness", "claude", *self.common)
        self.assertIn("--person person-from-env", self.command())
        run("install", "--harness", "claude", "--person", "person-flag", *self.common)
        self.assertIn("--person person-flag", self.command())

    def test_settings_keep_non_ascii_and_their_indent(self):
        self.claude.parent.mkdir()
        self.claude.write_text('{\n    "statusLine": "caf\u00e9 \u2014 ok"\n}\n')
        run("install", "--harness", "claude", *self.common)
        text = self.claude.read_text()
        self.assertIn("caf\u00e9 \u2014 ok", text)
        self.assertNotIn("\\u2014", text)
        self.assertTrue(text.startswith('{\n    "statusLine"'), "the file's 4-space indent is kept")

    def test_cy_is_not_pinned_to_its_resolved_target(self):
        real = self.root / "Cellar/camayoc/0.1.4/bin/cy"
        real.parent.mkdir(parents=True)
        real.write_text(STUB)
        real.chmod(0o755)
        link = self.root / "brew/bin/cy"
        link.parent.mkdir(parents=True)
        link.symlink_to(real)
        run("install", "--harness", "claude", "--mode", "block", "--cy", str(link),
            "--claude-settings", str(self.claude))
        cmd = self.command()
        self.assertIn(str(link), cmd)
        self.assertNotIn("Cellar", cmd)
        # an upgrade moves the real file: the symlinked path keeps working
        new = self.root / "Cellar/camayoc/0.1.5/bin/cy"
        new.parent.mkdir(parents=True)
        real.rename(new)
        link.unlink()
        link.symlink_to(new)
        code, out, _ = run("verify", "--harness", "claude", "--claude-settings", str(self.claude))
        self.assertEqual(0, code, out)
        # the install-time path gone entirely: the hook falls back to cy on PATH
        link.unlink()
        import subprocess
        payload = json.dumps(mrh._probe_payload("claude", self.root / "home/.claude/projects/p/memory/x.md"))
        env = {**os.environ, "PATH": f"{new.parent}:/usr/bin:/bin", "XDG_STATE_HOME": str(self.root / "st")}
        proc = subprocess.run(["sh", "-c", cmd], input=payload, capture_output=True, text=True, env=env)
        self.assertEqual(2, proc.returncode, proc.stderr)

    def test_off_until_installed(self):
        code, out, _ = run("status", "--harness", "both", *self.common)
        self.assertEqual(0, code)
        self.assertIn("claude: off", out)
        self.assertIn("codex: off", out)
        self.assertFalse(self.claude.exists() or self.codex.exists(), "status never creates files")

    def test_claude_install_keeps_other_hooks_and_is_idempotent(self):
        self.claude.parent.mkdir()
        other = {"matcher": "Bash", "hooks": [{"type": "command", "command": "dp signpost"}]}
        self.claude.write_text(json.dumps({"model": "x", "hooks": {"PreToolUse": [other]}}))
        for mode in ("copy", "block", "copy"):
            self.assertEqual(0, run("install", "--harness", "claude", "--mode", mode, "--person", "person-stiwi",
                                    *self.common)[0])
        data = json.loads(self.claude.read_text())
        self.assertEqual("x", data["model"])
        groups = data["hooks"]["PreToolUse"]
        self.assertEqual(other, groups[0])
        ours = [g for g in groups if g["matcher"] == "Write|Edit|MultiEdit"]
        self.assertEqual(1, len(ours))
        cmd = ours[0]["hooks"][0]["command"]
        self.assertIn("hook memory-redirect --mode copy --person person-stiwi", cmd)
        self.assertTrue(self.claude.with_name("settings.json.before-memory-redirect").exists())
        self.assertNotIn("memory-redirect", self.claude.with_name("settings.json.before-memory-redirect").read_text())
        _, out, _ = run("status", "--harness", "claude", *self.common)
        self.assertIn("claude: copy on Write|Edit|MultiEdit", out)
        self.assertEqual(0, run("uninstall", "--harness", "claude", *self.common)[0])
        self.assertEqual({"model": "x", "hooks": {"PreToolUse": [other]}}, json.loads(self.claude.read_text()))

    def test_codex_install_keeps_comments_and_uninstall_removes_only_its_block(self):
        self.codex.parent.mkdir()
        original = '# my settings\nmodel = "gpt"\n\n[[hooks.PreToolUse]]\nmatcher = "Bash"\n\n' \
                   '[[hooks.PreToolUse.hooks]]\ntype = "command"\ncommand = "guard"\n'
        self.codex.write_text(original)
        for mode in ("block", "copy"):
            self.assertEqual(0, run("install", "--harness", "codex", "--mode", mode, *self.common)[0])
        text = self.codex.read_text()
        self.assertTrue(text.startswith("# my settings\n"))
        self.assertEqual(1, text.count(mrh.BEGIN))
        data = tomllib.loads(text)
        self.assertEqual(["Bash", "apply_patch"], [g["matcher"] for g in data["hooks"]["PreToolUse"]])
        self.assertEqual(1, len(mrh.codex_entries(self.codex)))
        self.assertEqual("copy", mrh.mode_of(mrh.codex_entries(self.codex)[0]["command"]))
        self.assertEqual(0, run("uninstall", "--harness", "codex", *self.common)[0])
        self.assertEqual(original, self.codex.read_text())

    def test_refusals_change_nothing(self):
        self.claude.parent.mkdir()
        self.claude.write_text("{not json")
        code, _, err = run("install", "--harness", "claude", *self.common)
        self.assertEqual(2, code)
        self.assertIn("not valid JSON", err)
        self.assertEqual("{not json", self.claude.read_text())
        code, _, err = run("install", "--harness", "claude", "--cy", str(self.root / "nope"),
                           "--claude-settings", str(self.claude))
        self.assertEqual(2, code)
        self.assertIn("uv tool install", err)
        code, _, err = run("install", "--harness", "codex", "--person", "bad value; rm", *self.common)
        self.assertEqual(2, code)
        self.assertFalse(self.codex.exists())

    def test_hook_command_survives_a_missing_cy(self):
        cmd = mrh.hook_command(str(self.root / "gone/cy"), "block", None)
        import subprocess
        proc = subprocess.run(["sh", "-c", cmd], input="{}", capture_output=True, text=True)
        self.assertEqual((0, ""), (proc.returncode, proc.stderr), "a removed cy never breaks the agent")


class VerifyTests(Base):

    def test_verify_fails_when_not_installed(self):
        code, out, _ = run("verify", "--harness", "claude", *self.common)
        self.assertEqual(1, code)
        self.assertIn("FAIL claude: expected exactly one", out)

    def test_verify_proves_each_mode_fires_for_both_harnesses(self):
        for mode in ("copy", "block"):
            with self.subTest(mode=mode):
                self.assertEqual(0, run("install", "--harness", "both", "--mode", mode, *self.common)[0])
                code, out, _ = run("verify", "--harness", "both", *self.common)
                self.assertEqual(0, code, out)
                self.assertIn(f"OK claude: configured, {mode} mode", out)
                self.assertIn(f"OK codex: configured, {mode} mode on apply_patch", out)
                self.assertIn(f"fired in {mode} mode", out)

    def test_verify_catches_a_hook_that_does_not_fire_or_whose_worker_never_runs(self):
        self.assertEqual(0, run("install", "--harness", "claude", "--mode", "copy", *self.common)[0])
        os.environ["CAMAYOC_MEMORY_REDIRECT"] = "BROKEN"  # the stub then exits 0 without logging
        try:
            # verify strips the override, so it tests the configured mode, not this shell's
            self.assertEqual(0, run("verify", "--harness", "claude", *self.common)[0])
        finally:
            os.environ.pop("CAMAYOC_MEMORY_REDIRECT")
        self.cy.write_text(STUB.replace('write("fired"', 'None and write("fired"'))
        code, out, _ = run("verify", "--harness", "claude", *self.common)
        self.assertEqual(1, code)
        self.assertIn("logged no `fired` event", out)
        self.cy.write_text(STUB)
        os.environ["STUB_NO_WORKER"] = "1"
        orig = mrh.fire
        try:
            mrh.fire = lambda h, c, wait_s=1.0: orig(h, c, wait_s=1.0)
            code, out, _ = run("verify", "--harness", "claude", *self.common)
        finally:
            mrh.fire = orig
            os.environ.pop("STUB_NO_WORKER")
        self.assertEqual(1, code)
        self.assertIn("background worker logged nothing", out)


CLAUDE_STUB = textwrap.dedent("""\
    #!/usr/bin/env python3
    # A stand-in for `claude -p`: honours --settings PreToolUse hooks for one Write, like Claude Code.
    import json, os, re, subprocess, sys
    args = sys.argv[1:]
    here = os.path.dirname(os.path.abspath(__file__))
    # the live run passes a minimal environment, so the test talks to this stub through files
    real_home = open(os.path.join(here, "real-home")).read()
    assert not any(k.startswith("STUB_") for k in os.environ), "the live run leaked the caller's environment"
    assert os.environ["HOME"] != real_home and os.environ["CLAUDE_CONFIG_DIR"].startswith(os.environ["HOME"])
    assert args[args.index("--setting-sources") + 1] == ""
    os.makedirs(os.path.join(os.environ["CLAUDE_CONFIG_DIR"], "projects", "scratch-session"), exist_ok=True)
    target = re.search(r"create the file (\\S+) with", args[args.index("-p") + 1]).group(1)
    settings = json.load(open(args[args.index("--settings") + 1]))
    payload = json.dumps({"tool_name": "Write", "tool_input": {"file_path": target, "content": "x"}})
    allowed = True
    if not os.path.exists(os.path.join(here, "ignore-hooks")):
        for group in settings["hooks"]["PreToolUse"]:
            for h in group["hooks"]:
                if subprocess.run(["sh", "-c", h["command"]], input=payload, text=True).returncode == 2:
                    allowed = False
    if allowed:
        open(target, "w").write("x")
    print("DONE")
    """)


class LiveTests(Base):

    def setUp(self):
        super().setUp()
        self.bin = self.root / "claude-bin"
        self.bin.mkdir()
        stub = self.bin / "claude"
        stub.write_text(CLAUDE_STUB)
        stub.chmod(0o755)
        self.path = os.environ["PATH"]
        os.environ["PATH"] = f"{self.bin}:{self.path}"
        (self.bin / "real-home").write_text(str(Path.home()))
        os.environ["STUB_LEAK_CANARY"] = "1"
        run("install", "--harness", "claude", "--mode", "copy", *self.common)

    def tearDown(self):
        os.environ["PATH"] = self.path
        for k in ("STUB_LEAK_CANARY", "CLAUDE_CODE_OAUTH_TOKEN"):
            os.environ.pop(k, None)
        super().tearDown()

    def test_live_needs_an_explicit_credential(self):
        code, out, _ = run("verify", "--harness", "claude", "--live", *self.common)
        self.assertEqual(1, code)
        self.assertIn("scratch HOME, which has no login", out)

    def test_live_runs_in_a_scratch_home_with_only_the_hook(self):
        os.environ["CLAUDE_CODE_OAUTH_TOKEN"] = "fake-for-stub"
        code, out, _ = run("verify", "--harness", "claude", "--live", *self.common)
        self.assertEqual(0, code, out)
        self.assertIn("OK claude --live: Claude Code (scratch HOME, only this hook) invoked it 1x", out)
        (self.bin / "ignore-hooks").write_text("")
        code, out, _ = run("verify", "--harness", "claude", "--live", *self.common)
        self.assertEqual(1, code)
        self.assertIn("did not invoke the hook", out)


@unittest.skipUnless(os.environ.get("CAMAYOC_SRC"), "set CAMAYOC_SRC to a camayoc checkout to verify the real hook")
class RealCamayocTests(Base):

    def setUp(self):
        super().setUp()
        src = Path(os.environ["CAMAYOC_SRC"]).resolve()
        self.cy.write_text(f"#!/bin/sh\nPYTHONPATH={src} exec {sys.executable} -m camayoc_cli \"$@\"\n")

    def test_real_hook_installs_and_verifies_in_both_modes(self):
        for mode in ("copy", "block"):
            with self.subTest(mode=mode):
                self.assertEqual(0, run("install", "--harness", "both", "--mode", mode, "--person", "person-stiwi",
                                        *self.common)[0])
                code, out, _ = run("verify", "--harness", "both", *self.common)
                print(out)
                self.assertEqual(0, code, out)


if __name__ == "__main__":
    unittest.main()
