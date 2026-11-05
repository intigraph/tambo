"""Source-only reproducer; every native command is mocked."""
import hashlib
from pathlib import Path
import subprocess
import types
from unittest.mock import patch

BASE = '7fd2777'
FILE = 'scripts/macos-controller-account-packet.py'
candidate = Path(FILE).read_text()
for label, source in [
    ('BASE ' + BASE, subprocess.check_output(['git', 'show', BASE + ':' + FILE], text=True)),
    ('CANDIDATE script sha256 ' + hashlib.sha256(candidate.encode()).hexdigest(), candidate),
]:
    print(label, flush=True)
    module = types.ModuleType('packet')
    exec(compile(source, FILE, 'exec'), module.__dict__)
    for output in ['root 0\nnobody -2\nnogroup -1\n',
                   'nobody -2\nnobody -1\n', 'nobody --2\n', '']:
        with patch.object(module, 'run', return_value=types.SimpleNamespace(stdout=output)):
            try:
                print(repr(output), '->', module.inventory('/Groups', 'PrimaryGroupID'), flush=True)
            except ValueError as error:
                print(repr(output), '-> REFUSED:', error, flush=True)
