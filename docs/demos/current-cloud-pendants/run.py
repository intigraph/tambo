"""Run the actual optional cloud sync block with only a synthetic current executable."""
import importlib.util
import json
import os
from pathlib import Path
import tempfile

# The fixture owns and removes its tiny temporary project and executable.
os.environ["TMPDIR"] = "/tmp"
tempfile.tempdir = None
root = Path(__file__).resolve().parents[3]
spec = importlib.util.spec_from_file_location("setup_control", root / "tests/test_pendant_setup.py")
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)
status, calls, repo, authority = module.PendantSetup().probe()
assert calls == ["packs", "sync", "--repo", repo, "--shapes-from", authority]
assert "sync completed" in status
print(json.dumps({"available_executable": "tambo", "calls": calls, "status": status.strip()}, indent=2))
