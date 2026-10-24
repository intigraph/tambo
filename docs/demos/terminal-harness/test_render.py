#!/usr/bin/env python3
"""Independent leak and fail-closed capture controls; no real tool is stubbed."""
import importlib.util
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location("recorder", HERE / "render.py")
recorder = importlib.util.module_from_spec(spec)
spec.loader.exec_module(recorder)


class CaptureControls(unittest.TestCase):
    def test_public_text_and_cli_flags_pass(self):
        recorder.check_text("$ quipu episode fixture.json --base-ns urn:demo:\nAda studies Engine")

    def test_private_text_refuses(self):
        for text in ["/" + "home/person/file", "fixture" + ".svc", "192.0.2.1",
                     "Bearer abcdef", "token=abc", "aegis-example", "\x1b[31m"]:
            with self.subTest(text=text), self.assertRaises(ValueError):
                recorder.check_text(text)

    def test_tool_specific_version_command_is_used(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            (root / ".terminal-demo-sandbox").touch()
            tool = root / "fixture-tool"
            tool.write_text('#!/usr/bin/python3\nimport sys\n'
                            'if "--version" in sys.argv: sys.exit(2)\n'
                            'print("fixture 2" if "version" in sys.argv else "ok")\n')
            tool.chmod(0o700)
            tape = {"title": "Public fixture", "description": "Control",
                    "width": 1000, "height": 620, "font_size": 24,
                    "line_height": 32, "version_args": {"fixture": ["version"]},
                    "steps": [{"tool": "fixture", "args": ["run"],
                               "requires": ["ok"], "hold": 10}]}
            path = root / "tape.json"
            path.write_text(json.dumps(tape))
            subprocess.run([sys.executable, str(HERE / "render.py"), str(path),
                            "--workspace", str(root), "--output", str(root / "out"),
                            "--tool", "fixture=" + str(tool)], check=True, capture_output=True)
            manifest = json.loads((root / "out/manifest.json").read_text())
            self.assertEqual(manifest["versions"]["fixture"], "fixture 2")
            self.assertEqual(manifest["version_args"]["fixture"], ["version"])

    def test_failures_publish_nothing_and_keep_prior_asset(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            tool = root / "fixture-tool"
            tool.write_text('#!/usr/bin/python3\nimport sys\n'
                            'print("fixture 1" if "--version" in sys.argv else sys.argv[1])\n'
                            'sys.exit(3 if "bad-exit" in sys.argv else 0)\n')
            tool.chmod(0o700)
            out = root / "out"
            out.mkdir()
            (out / "demo.svg").write_text("prior")
            tape = {"title": "Public fixture", "description": "Control",
                    "width": 1000, "height": 620, "font_size": 24,
                    "line_height": 32, "steps": []}
            for marker, value, required in [(False, "ok", []), (True, "bad-exit", []),
                                             (True, "fixture" + ".svc", []),
                                             (True, "ok", ["absent"])]:
                if marker:
                    (root / ".terminal-demo-sandbox").touch()
                tape["steps"] = [{"tool": "fixture", "args": [value],
                                   "requires": required, "hold": 10}]
                path = root / "tape.json"
                path.write_text(json.dumps(tape))
                result = subprocess.run([sys.executable, str(HERE / "render.py"), str(path),
                                         "--workspace", str(root), "--output", str(out),
                                         "--tool", "fixture=" + str(tool)], capture_output=True)
                self.assertNotEqual(result.returncode, 0)
                self.assertEqual((out / "demo.svg").read_text(), "prior")
                self.assertFalse((out / "manifest.json").exists())


if __name__ == "__main__":
    unittest.main()
