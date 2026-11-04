#!/usr/bin/env python3
"""Exercise the actual probe with private fixture installers; no network or packages."""
import hashlib
import importlib.util
import io
import json
from pathlib import Path
import tempfile
from unittest.mock import patch

spec = importlib.util.spec_from_file_location('probe', Path(__file__).with_name('probe-public-installs.py'))
probe = importlib.util.module_from_spec(spec)
spec.loader.exec_module(probe)
row = dict(repository='fixture/tool', release_tag='v1.2.3', source_sha='fixture',
           command='curl -fsSL https://raw.githubusercontent.com/fixture/tool/main/install.sh | sh',
           commands=['fixture-tool'], kind='cli')
script = b'''set -eu
[ -z "${GITHUB_TOKEN:-}${QUIPU_AUTH_TOKEN:-}${SEEDS_ACTOR:-}" ]
mkdir -p "$HOME/.local/bin"
printf '#!/bin/sh\\nprintf "fixture-tool 1.2.3\\\\n"\\n' > "$HOME/.local/bin/fixture-tool"
chmod +x "$HOME/.local/bin/fixture-tool"
'''
row['installer_sha256'] = hashlib.sha256(script).hexdigest()

def case(name, changes, payload, expected):
    with tempfile.TemporaryDirectory(prefix='probe-control-') as directory:
        with patch.object(probe.urllib.request, 'urlopen', return_value=io.BytesIO(payload)), patch.object(probe, 'latest_release', return_value=changes.get('release_tag', row['release_tag'])):
            result = probe.probe(dict(row, **changes), Path(directory))
        assert result['outcome'] == expected, (name, result)
        print('PASS', name, expected)

case('actual isolated install/version positive', {}, script, 'VERSION_VERIFIED')
case('missing advertised command', {'command':None}, script, 'MISSING_DOCUMENTED_ONE_LINER')
case('changed script refused before execution', {}, script+b'# changed\n', 'UNKNOWN_SCRIPT_CHANGED')
case('nonzero installer retained', {'installer_sha256':hashlib.sha256(b'exit 7\n').hexdigest()}, b'exit 7\n', 'INSTALL_FAILED')
case('wrong release version refused', {'release_tag':'v1.2.4'}, script, 'VERSION_MISMATCH')
case('system command cannot impersonate install', {'commands':['sh']}, script, 'INSTALLED_COMMAND_MISSING_OR_SHADOWED')
with patch.dict(probe.os.environ, {'GITHUB_TOKEN':'fixture-never-forward','QUIPU_AUTH_TOKEN':'fixture-never-forward','SEEDS_ACTOR':'fixture-never-forward'}):
    case('caller credentials and identity not forwarded', {}, script, 'VERSION_VERIFIED')
assert probe.release_version('seeds-ai-v0.12.0') == '0.12.0'
print('PASS tagged release version parser')
