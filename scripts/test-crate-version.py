#!/usr/bin/env python3
"""Offline controls for the actual registry verifier's three-state contract."""
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile

checker = Path(__file__).resolve().with_name("check-crate-version.sh")
with tempfile.TemporaryDirectory() as directory:
    root = Path(directory)
    curl = root / "curl"
    curl.write_text(f"#!{sys.executable}\n" + '''import os, pathlib, sys
args = sys.argv[1:]
assert args[-1] == "https://crates.io/api/v1/crates/tambo/0.2.23"
pathlib.Path(args[args.index("-o") + 1]).write_text(os.environ["MOCK_BODY"])
print(os.environ["MOCK_STATUS"], end="")
sys.exit(int(os.environ["MOCK_EXIT"]))
''')
    curl.chmod(0o755)
    good = {"version": {"crate": "tambo", "num": "0.2.23", "yanked": False}}
    cases = [
        ("present", "200", good, 0, 0),
        ("missing", "404", {"errors": []}, 0, 1),
        ("forbidden", "403", {}, 0, 2),
        ("timeout", "000", {}, 28, 2),
        ("wrong crate", "200", {"version": {**good["version"], "crate": "other"}}, 0, 2),
        ("wrong version", "200", {"version": {**good["version"], "num": "0.2.22"}}, 0, 2),
        ("yanked", "200", {"version": {**good["version"], "yanked": True}}, 0, 2),
        ("missing fields", "200", {}, 0, 2),
        ("invalid JSON", "200", "invalid", 0, 2),
    ]
    for name, status, body, curl_exit, expected in cases:
        env = {**os.environ, "PATH": str(root) + os.pathsep + os.environ["PATH"],
               "MOCK_STATUS": status, "MOCK_BODY": body if isinstance(body, str) else json.dumps(body),
               "MOCK_EXIT": str(curl_exit)}
        result = subprocess.run([str(checker), "tambo", "0.2.23"], env=env, capture_output=True, text=True)
        assert result.returncode == expected, (name, result.returncode, result.stderr)
        print(f"PASS: {name} -> {expected}")
    invalid = subprocess.run([str(checker), "tambo", "..."], capture_output=True)
    assert invalid.returncode == 2
    print("PASS: invalid version -> UNKNOWN")
