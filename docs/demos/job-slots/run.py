"""Read-only account-plan CLI fixture. No --apply or native account operation."""
import hashlib
import json
from pathlib import Path
import subprocess
import tempfile

principal = 'eph-task-123abc'
with tempfile.TemporaryDirectory(dir='.', prefix='job-plan-fixture-') as directory:
    root = Path(directory)
    raw = json.dumps(dict(version=1, principal=principal, uid=51001, gid=51001,
                         controller_uid=51000, username='_stjobs_123abc',
                         home='/var/lib/shantytown/agents/crew-' + principal,
                         root='/var/lib/shantytown/job-stores/' + principal)).encode()
    manifest = root / (principal + '.json')
    manifest.write_bytes(raw)
    (root / 'slots.json').write_text(json.dumps(dict(version=1, uid_min=51001, uid_max=51016,
        slots={principal: dict(uid=51001, sha256=hashlib.sha256(raw).hexdigest())})))
    for binary, expected in [('demo-bin/tambo-base', 2), ('demo-bin/tambo-head', 0)]:
        print('$ ' + binary + ' job-slots --directory <disposable-fixture>', flush=True)
        answer = subprocess.run([binary, 'job-slots', '--directory', str(root)], text=True, capture_output=True)
        print(answer.stdout + answer.stderr, end='', flush=True)
        assert answer.returncode == expected
    manifest.write_text('{}')
    print('$ demo-bin/tambo-head job-slots --directory <tampered-fixture>', flush=True)
    answer = subprocess.run(['demo-bin/tambo-head', 'job-slots', '--directory', str(root)],
                            text=True, capture_output=True)
    print(answer.stdout + answer.stderr, end='', flush=True)
    assert answer.returncode != 0 and 'digest differs' in answer.stderr
    print('No apply operation: planned account only; tampered manifest refused.', flush=True)
