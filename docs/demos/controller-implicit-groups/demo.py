#!/usr/bin/env python3
"""Offline native_verify before/after; every directory/account command is mocked."""
import importlib.util
from pathlib import Path
import subprocess
import tempfile
import time

ROOT = Path(__file__).resolve().parents[3]
BASE = "7d1a175499cd8968a0ca185c17cd6b92fa9e67b5"

def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module

controls = load("controls", ROOT / "scripts/test-macos-controller-account-packet.py")
candidate = controls.p
base = subprocess.check_output(["git", "show", BASE + ":scripts/macos-controller-account-packet.py"], cwd=ROOT)
head = subprocess.check_output(["git", "rev-parse", "--short=12", "HEAD"], cwd=ROOT, text=True).strip()
print("Offline command fixtures; no native accounts, sudo, host files or receipt writes.", flush=True)
print("Mac-shaped id -G controller: 51000 12 61 701 100", flush=True)
print("Baseline nobody: 4294967294 12 61 701 100; primary 4294967294", flush=True)
with tempfile.TemporaryDirectory() as temporary:
    path = Path(temporary) / "before.py"
    path.write_bytes(base)
    before = load("before", path)
    for label, module, expected in [("BASE " + BASE[:12], before, False),
                                    ("CANDIDATE " + head + " + working source", candidate, True)]:
        controls.p = module
        with controls.ControllerTests().macos_groups():
            try:
                module.native_verify(controls.ROW)
            except ValueError as error:
                print(label + " -> REFUSED: " + str(error), flush=True)
                assert not expected
            else:
                print(label + " -> VERIFIED: dedicated primary group, implicit baseline, no explicit grant", flush=True)
                assert expected
        time.sleep(1)
controls.p = candidate
for kind in ["staff", "explicit name", "explicit UUID"]:
    with controls.ControllerTests().macos_groups() as (state, _):
        if kind == "staff":
            state["_stctl"] += " 20"
        elif kind == "explicit name":
            state["names"] = "everyone GroupMembership = _stctl"
        else:
            state["guids"] = "localaccounts GroupMembers = 11111111-2222-3333-4444-555555555555"
        try:
            candidate.native_verify(controls.ROW)
        except ValueError as error:
            print("CONTROL " + kind + " -> REFUSED: " + str(error), flush=True)
        else:
            raise AssertionError("unsafe explicit/shared group admitted")
    time.sleep(1)
