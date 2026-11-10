#!/usr/bin/env python3
"""Show the verifier's episode boundary in a disposable local store."""
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile

cli = shutil.which('quipu')
if not cli:
    raise SystemExit('quipu is required')
env = {key: value for key, value in os.environ.items() if not key.startswith('QUIPU_')}
with tempfile.TemporaryDirectory() as tmp:
    root = Path(tmp)
    (root / 'episode.json').write_text(json.dumps({
        'name': 'Tambo verification', 'episode_body': 'functional round trip',
        'source': 'caboodle', 'group_id': 'caboodle-verification',
        'nodes': [{'name': 'tambo-verify-roundtrip', 'type': 'Verification',
                   'description': 'Tambo isolated verification'}], 'edges': []}))
    args = ['episode', 'episode.json', '--db', 'verify.db']
    print('$ quipu ' + ' '.join(args), flush=True)
    before = subprocess.run([cli, *args], cwd=root, env=env, capture_output=True, text=True)
    print(before.stderr.strip(), flush=True)
    assert before.returncode != 0 and 'no usable base namespace' in before.stderr
    print('expected refusal: exit ' + str(before.returncode), flush=True)
    args += ['--base-ns', 'https://example.org/tambo/verification/']
    print('\n$ quipu ' + ' '.join(args), flush=True)
    after = subprocess.run([cli, *args], cwd=root, env=env, capture_output=True, text=True)
    print((after.stdout + after.stderr).strip(), flush=True)
    assert after.returncode == 0
    query = ('SELECT ?label WHERE { ?s <http://www.w3.org/2000/01/rdf-schema#label> ?label . '
             'FILTER(?label = "tambo-verify-roundtrip") }')
    print('\n$ quipu read <exact marker query> --db verify.db', flush=True)
    reader = subprocess.run([cli, 'read', query, '--db', 'verify.db'], cwd=root,
                            env=env, capture_output=True, text=True)
    print(reader.stdout.strip(), flush=True)
    assert reader.returncode == 0 and 'tambo-verify-roundtrip' in reader.stdout
