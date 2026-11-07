#!/usr/bin/env python3
"""Exercise the actual probe with private fixture installers; no network or packages."""
import hashlib
import importlib.util
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
[ -z "${GITHUB_TOKEN:-}${GH_TOKEN:-}${CENSUS_GITHUB_METADATA_TOKEN:-}${QUIPU_AUTH_TOKEN:-}${SEEDS_ACTOR:-}" ]
mkdir -p "$HOME/.local/bin"
printf '#!/bin/sh\\nprintf "fixture-tool 1.2.3\\\\n"\\n' > "$HOME/.local/bin/fixture-tool"
chmod +x "$HOME/.local/bin/fixture-tool"
'''
row['installer_sha256'] = hashlib.sha256(script).hexdigest()

def case(name, changes, payload, expected, alias=False):
    with tempfile.TemporaryDirectory(prefix='probe-control-') as directory:
        workspace = Path(directory)
        if alias:
            real = workspace / 'real'
            real.mkdir()
            workspace = workspace / 'alias'
            workspace.symlink_to(real, target_is_directory=True)
        def download(url, destination, environment, workspace):
            assert not {"GITHUB_TOKEN", "GH_TOKEN", "CENSUS_GITHUB_METADATA_TOKEN", "QUIPU_AUTH_TOKEN", "SEEDS_ACTOR"}.intersection(environment)
            destination.write_bytes(payload)
            return 0, ''
        with patch.object(probe, 'fetch_installer', side_effect=download), patch.object(probe, 'latest_release', return_value=changes.get('release_tag', row['release_tag'])):
            result = probe.probe(dict(row, **changes), workspace)
        assert result['outcome'] == expected, (name, result)
        print('PASS', name, expected)

case('actual isolated install/version positive', {}, script, 'VERSION_VERIFIED')
case('missing advertised command', {'command':None}, script, 'MISSING_DOCUMENTED_ONE_LINER')
case('changed script refused before execution', {}, script+b'# changed\n', 'UNKNOWN_SCRIPT_CHANGED')
case('nonzero installer retained', {'installer_sha256':hashlib.sha256(b'exit 7\n').hexdigest()}, b'exit 7\n', 'INSTALL_FAILED')
case('wrong release version refused', {'release_tag':'v1.2.4'}, script, 'VERSION_MISMATCH')
for name, body, expected in [
    ('version command failure is not a mismatched release', 'exit 2', 'VERSION_COMMAND_FAILED'),
    ('missing version output remains unproven', 'echo tool-help', 'VERSION_NOT_REPORTED'),
]:
    payload = ('mkdir -p "$HOME/.local/bin"\n'
               'printf \'#!/bin/sh\\n' + body + '\\n\' > "$HOME/.local/bin/fixture-tool"\n'
               'chmod +x "$HOME/.local/bin/fixture-tool"\n').encode()
    case(name, {'installer_sha256':hashlib.sha256(payload).hexdigest()}, payload, expected)
case('system command cannot impersonate install', {'commands':['sh']}, script, 'INSTALLED_COMMAND_MISSING_OR_SHADOWED')
with patch.dict(probe.os.environ, {'CENSUS_GITHUB_METADATA_TOKEN':'fixture_api_token','GH_TOKEN':'fixture-never-forward','GITHUB_TOKEN':'fixture-never-forward','QUIPU_AUTH_TOKEN':'fixture-never-forward','SEEDS_ACTOR':'fixture-never-forward'}):
    case('caller credentials and identity not forwarded', {}, script, 'VERSION_VERIFIED')
assert probe.release_version('seeds-ai-v0.12.0') == '0.12.0'
print('PASS tagged release version parser')
case('private install through aliased temporary root', {}, script, 'VERSION_VERIFIED', alias=True)
case('aliased home still excludes system shadow', {'commands':['sh']}, script,
     'INSTALLED_COMMAND_MISSING_OR_SHADOWED', alias=True)

# Metadata credentials belong only to the parent API request, never installers.
import io
import urllib.error
import urllib.request

with tempfile.TemporaryDirectory(prefix='metadata-control-') as directory:
    requests = []
    class MetadataOpener:
        def open(self, request, data=None, timeout=None):
            requests.append((request, timeout))
            return io.BytesIO(b'{"tag_name":"v1.2.3"}')
    with patch.dict(probe.os.environ, {'CENSUS_GITHUB_METADATA_TOKEN':'fixture_api_token'}), patch.object(
            probe.urllib.request, 'build_opener', return_value=MetadataOpener()):
        assert probe.latest_release('fixture/tool') == 'v1.2.3'
    request, timeout = requests.pop()
    assert isinstance(request, urllib.request.Request), 'metadata request has no explicit authentication headers'
    assert request.full_url == 'https://api.github.com/repos/fixture/tool/releases/latest'
    assert request.get_header('Authorization') == 'Bearer fixture_api_token'
    assert timeout == 30
print('PASS metadata-only authenticated parent request')

with patch.dict(probe.os.environ, {'CENSUS_GITHUB_METADATA_TOKEN':''}), patch.object(
        probe.urllib.request, 'build_opener', return_value=MetadataOpener()):
    assert probe.latest_release('fixture/tool') == 'v1.2.3'
assert requests.pop()[0].get_header('Authorization') is None
print('PASS anonymous metadata request remains credential-free')

handler = probe.GitHubMetadataRedirect()
original = urllib.request.Request('https://api.github.com/repos/old/tool/releases/latest',
                                  headers={'Authorization':'Bearer fixture_api_token'})
redirected = handler.redirect_request(original, None, 301, 'moved', {},
    'https://api.github.com/repos/new/tool/releases/latest')
assert redirected.get_header('Authorization') == 'Bearer fixture_api_token'
print('PASS same-API repository redirect retains metadata identity')
for target in ['https://example.invalid/steal', 'http://api.github.com/repos/tool',
               'https://api.github.com@evil.invalid/steal', 'https://api.github.com.evil.invalid/steal']:
    try:
        handler.redirect_request(original, None, 302, 'moved', {}, target)
    except ValueError as error:
        assert str(error) == 'metadata redirect outside GitHub API refused'
    else:
        raise AssertionError('metadata credential redirect escaped API origin')
print('PASS cross-origin and plaintext metadata redirects refused')
for invalid in ['fixture/tool?other=1', 'fixture/tool/extra', '//evil.invalid']:
    try:
        probe.latest_release(invalid)
    except ValueError as error:
        assert str(error) == 'invalid GitHub repository identity'
    else:
        raise AssertionError('invalid repository accepted')
print('PASS metadata repository path injection refused before HTTP')
with patch.dict(probe.os.environ, {'CENSUS_GITHUB_METADATA_TOKEN':'fixture\nsecret'}):
    try:
        probe.latest_release('fixture/tool')
    except ValueError as error:
        assert str(error) == 'metadata token has invalid header bytes'
        assert 'secret' not in str(error)
    else:
        raise AssertionError('invalid header bytes accepted')
print('PASS invalid metadata credential refused without its value')

with tempfile.TemporaryDirectory(prefix='metadata-failure-control-') as directory:
    def private_download(url, destination, environment, workspace):
        destination.write_bytes(script)
        return 0, ''
    with patch.object(probe, 'fetch_installer', side_effect=private_download), patch.object(
            probe, 'latest_release', side_effect=urllib.error.HTTPError(
                'https://api.github.com/repos/fixture/tool/releases/latest', 403, 'Forbidden', {}, None)), patch.object(
            probe, 'run', side_effect=AssertionError('installer executed after unknown metadata')):
        result = probe.probe(row, Path(directory))
    assert result['outcome'] == 'LATEST_RELEASE_UNKNOWN'
    assert 'install_exit' not in result
print('PASS metadata refusal remains UNKNOWN without installer execution')
