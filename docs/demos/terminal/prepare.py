#!/usr/bin/env python3
"""Prepare a new private HOME using a caller-supplied checksummed Tambo release."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess

p = argparse.ArgumentParser(description=__doc__)
p.add_argument("--tambo", type=Path, required=True)
p.add_argument("--workspace", type=Path, required=True)
a = p.parse_args()
binary = a.tambo.resolve(strict=True)
root = a.workspace.resolve()
root.mkdir()  # Never reuse, reset or delete an existing install/state.
(root / ".terminal-demo-sandbox").touch()
home = root / ".demo-home"
bin_dir = home / ".cargo/bin"
bin_dir.mkdir(parents=True)
shutil.copy2(binary, bin_dir / "tambo")
shutil.copy2(binary, bin_dir / "caboodle")
for source in (Path(__file__).parent / "fixture").iterdir():
    shutil.copy2(source, root / source.name)
(root / "answers.txt").write_text("\n".join(json.loads((root / "answers.json").read_text())) + "\n")
env = {"PATH": str(bin_dir) + os.pathsep + os.defpath, "HOME": str(home),
       "XDG_CONFIG_HOME": str(home / ".config"),
       "XDG_CACHE_HOME": str(home / ".cache"),
       "XDG_STATE_HOME": str(home / ".state"), "TMPDIR": str(root),
       "LANG": "C.UTF-8", "TERM": "dumb", "NO_COLOR": "1"}
logs = []
for args, stdin in [(["init", "--guided"], (root / "answers.txt").read_text()),
                    (["doctor"], None), (["apply"], None), (["verify"], None)]:
    r = subprocess.run([str(bin_dir / "tambo"), *args], cwd=root, env=env,
                       input=stdin, text=True, capture_output=True, timeout=300)
    logs.append({"args": args, "exit": r.returncode,
                 "stdout": r.stdout, "stderr": r.stderr})
    (root / "setup-log.json").write_text(json.dumps(logs, indent=2) + "\n")
    if r.returncode:
        raise SystemExit("sandbox preparation refused; retained setup-log.json; no recording")
shutil.copy2(root / "caboodle-plan.toml", root / "setup-plan.toml")
(root / "setup.provenance.json").write_text(json.dumps({
    "tambo_sha256": hashlib.sha256(binary.read_bytes()).hexdigest(),
    "plan_sha256": hashlib.sha256((root / "setup-plan.toml").read_bytes()).hexdigest(),
    "versions": {name: subprocess.check_output([str(bin_dir / name), "--version"],
                  cwd=root, env=env, text=True).strip() for name in ("tambo", "quipu")},
    "camayoc_revision": next((home / ".local/share/caboodle/camayoc").glob("*/REVISION")).read_text().strip()
    }, indent=2) + "\n")
print("Sandbox prepared; install and verification logs retained inside it.")
