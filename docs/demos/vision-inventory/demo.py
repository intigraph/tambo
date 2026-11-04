#!/usr/bin/env python3
"""Equivalent disposable vision/ledger fixtures; never rewrite the real ledger."""
import hashlib
import json
from pathlib import Path
import subprocess
import sys
from tempfile import TemporaryDirectory
ROOT = Path(__file__).resolve().parents[3]
BASE = '3c6688ff6b3b117185a5bafc8dd2776f80be663f'
FILE = 'docs/design/vision.md'
LEDGER = 'docs/compatibility/legacy-name-contracts.json'
old = json.loads(subprocess.check_output(['git','show',BASE+':'+LEDGER],cwd=ROOT))
new = json.loads((ROOT/LEDGER).read_text())
assert set(old['files']) == set(new['files'])
for name in old['files']:
    if name != FILE:
        assert old['files'][name] == new['files'][name]
removed = set(old['files'][FILE]['lines']) - set(new['files'][FILE]['lines'])
assert removed == {'235f0a6f388ba63bd1012b7a3d3e468e7300bb228a7d1857db81e117db38973c'}
assert set(new['files'][FILE]['lines']) < set(old['files'][FILE]['lines'])
content = subprocess.check_output(['git','show',BASE+':'+FILE],cwd=ROOT,text=True)
assert content == (ROOT/FILE).read_text()
print('BASE '+BASE+' -> CANDIDATE exact obsolete-entry removal')
with TemporaryDirectory() as directory:
    fixture = Path(directory)
    subprocess.run(['git','init','--quiet',str(fixture)],check=True)
    vision = fixture/FILE; vision.parent.mkdir(parents=True); vision.write_text(content)
    ledger = fixture/LEDGER; ledger.parent.mkdir(parents=True)
    def probe(label, entry, expected):
        ledger.write_text(json.dumps({'version':1,'files':{FILE:entry}}))
        run = subprocess.run([sys.executable,'-B',str(ROOT/'scripts/check-product-name.py'),
                              '--root',str(fixture)],capture_output=True,text=True)
        print(label+': exit='+str(run.returncode))
        print(run.stdout.strip())
        assert run.returncode == expected, run.stderr
    probe('Before',old['files'][FILE],1)
    probe('After',new['files'][FILE],0)
    retained = next(line for line in content.splitlines() if hashlib.sha256(line.encode()).hexdigest() in new['files'][FILE]['lines'])
    vision.write_text(content+'\nUnexpected '+retained+'\n')
    probe('Unreviewed-line negative control',new['files'][FILE],1)
print('PASS: one obsolete allowance removed; new references still refused')
