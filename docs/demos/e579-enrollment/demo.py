#!/usr/bin/env python3
"""Offline rendered candidates + bounded fake receiver, no native/live actions."""
import importlib.util
import json
from pathlib import Path
import plistlib
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / 'scripts'))
def module(name, file):
    spec = importlib.util.spec_from_file_location(name, ROOT / 'scripts' / file)
    result = importlib.util.module_from_spec(spec); spec.loader.exec_module(result)
    return result

renderer = module('renderer_fixture', 'test-macos-principal-seeds.py')
budget = module('budget_fixture', 'test-e579-bounded-delivery.py')
print('Target: source checkout; fixtures only, no native keys/accounts/sudo/launchd/network')
print('Base62b2727: key-init/config renderer and job-local bounded delivery absent from that tree')
print('Candidate source0b41557: the following actual local controls')
with tempfile.TemporaryDirectory() as directory:
    root = Path(directory); row = renderer.inputs()
    source = root / 'input.json'; source.write_text(json.dumps(row))
    result = subprocess.run([sys.executable, '-B', str(ROOT / 'scripts/render-macos-principal-seeds.py'),
                             'key-init', '--input', str(source), '--output', str(root / 'out')],
                            capture_output=True, text=True)
    print('render key-init exit=' + str(result.returncode)); print(result.stdout.strip())
    assert result.returncode == 0
    job = plistlib.loads(next((root / 'out').glob('*.plist')).read_bytes())
    print(json.dumps({k: job[k] for k in ('UserName', 'GroupName', 'Disabled', 'RunAtLoad', 'KeepAlive')}))
    row['uid'] = 501; source.write_text(json.dumps(row))
    result = subprocess.run([sys.executable, '-B', str(ROOT / 'scripts/render-macos-principal-seeds.py'),
                             'key-init', '--input', str(source), '--output', str(root / 'bad')],
                            capture_output=True, text=True)
    print('wrong-UID render exit=' + str(result.returncode) + ': ' + result.stderr.strip())
    assert result.returncode == 1 and not (root / 'bad').exists()
    path, _ = budget.BudgetTests().fixture(root)
    for attempt in (1, 2, 3):
        result = budget.BudgetTests().run_job(path)
        print('fake receiver invocation ' + str(attempt) + ' exit=' + str(result.returncode))
        print((result.stdout + result.stderr).strip())
        assert result.returncode == (75 if attempt < 3 else 1)
    assert (root / 'calls').read_text() == '11'
    print('Observed fake receiver child calls=2; third child withheld; readiness FALSE')
