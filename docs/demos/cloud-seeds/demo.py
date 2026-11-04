#!/usr/bin/env python3
"""Compare cloud bootstrap using the same private, network-free fixture."""
import importlib.util
from pathlib import Path
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[3]
BASE = '76f63a2e00b01d92c30c99c86a59f02dc5160572'
spec = importlib.util.spec_from_file_location('bootstrap', ROOT / 'scripts/test-install-bootstrap.py')
fixture = importlib.util.module_from_spec(spec)
spec.loader.exec_module(fixture)


def observe(label, source, expect_legacy):
    fixture.ROOT = source
    case = fixture.CloudEnvironment('test_missing_seeds_installs_release_instead_of_beads')
    case.setUp()
    try:
        failed = False
        try:
            case.test_missing_seeds_installs_release_instead_of_beads()
        except AssertionError:
            failed = True
        calls = case.calls.read_text() if case.calls.exists() else ''
        requests = case.requests.read_text() if case.requests.exists() else ''
        legacy = '@beads/bd' in calls
        seeds = case.url in requests
        print(f'{label}: legacy npm install attempted={legacy}; Seeds release requested={seeds}')
        assert (failed, legacy, seeds) == (expect_legacy, expect_legacy, not expect_legacy)
    finally:
        case.doCleanups()


print('Target: source checkout, synthetic empty-tool fixture; no network or board writes.')
with tempfile.TemporaryDirectory() as temporary:
    before = Path(temporary)
    (before / 'scripts').mkdir()
    (before / 'scripts/setup-environment.sh').write_bytes(subprocess.check_output(
        ['git', 'show', BASE + ':scripts/setup-environment.sh'], cwd=ROOT))
    observe('BEFORE ' + BASE[:8], before, True)
observe('AFTER candidate bootstrap', ROOT, False)
fixture.ROOT = ROOT
control = fixture.CloudEnvironment('test_both_known_seeds_identities_skip_installation')
result = control.run()
assert result.wasSuccessful(), (result.errors, result.failures)
print('Positive control: both installed Seeds identities skip the download (verified).')
