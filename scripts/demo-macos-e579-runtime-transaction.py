#!/usr/bin/env python3
"""Synthetic before/after; no native commands, privilege or live paths."""
import importlib.util
from pathlib import Path


def load(name):
    spec = importlib.util.spec_from_file_location(name, Path(__file__).with_name(name + '.py'))
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


fixture = load('test-macos-e579-runtime-transaction')
case = fixture.Controls('test_full_forward_and_inverse_preserves_both_versions')
case.setUp()
try:
    old = load('macos-controller-account-packet')
    old.protected = case.tx.safe
    print('SYNTHETIC public bytes; base create-only installer, candidate finite transaction')
    print('Candidate source SHA256:', fixture.m.sha(Path(fixture.m.__file__).read_bytes()))
    try:
        old.install(case.tx.tool, case.new_tool, 0o755)
    except ValueError:
        print('BEFORE: create-only install REFUSED occupied different tool; old bytes retained')
    else:
        raise AssertionError('negative control failed')
    case.tx.a1(case.tool)
    case.tx.a2(case.archive, case.stage)
    case.tx.tree(case.tx.runtime, fixture.m.NEW_RUNTIME)
    print('AFTER: guarded tool/runtime transition verified; both old backup/restore copies retained')
    case.tx.a3()
    case.tx.tree(case.tx.runtime, fixture.m.OLD_RUNTIME)
    assert case.tx.tool.read_bytes() == case.old_tool
    print('INVERSE: exact old tool/runtime restored; new bytes retained; readiness remains FALSE')
    try:
        case.tx.a1(case.tool)
    except ValueError:
        print('REPLAY: refused; no deletion or automatic recovery')
    else:
        raise AssertionError('replay control failed')
finally:
    case.doCleanups()
