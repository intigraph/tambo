#!/usr/bin/env python3
"""Measure existing documented installers in disposable CI homes; never provision services."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import shutil
import signal
import subprocess
import sys
import tempfile
import time
import urllib.request
import urllib.parse


def release_version(tag):
    match = re.search(r'\d+\.\d+\.\d+(?:-[A-Za-z0-9.-]+)?', tag)
    return match.group() if match else None


def run(command, environment, directory, timeout=240):
    with tempfile.TemporaryFile() as output:
        process = subprocess.Popen(command, env=environment, cwd=directory,
                                   stdout=output, stderr=subprocess.STDOUT,
                                   start_new_session=True)
        try:
            code = process.wait(timeout=timeout)
        except subprocess.TimeoutExpired:
            os.killpg(process.pid, signal.SIGTERM)
            try:
                process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                os.killpg(process.pid, signal.SIGKILL)
                process.wait()
            code = 124
        output.seek(0)
        return code, output.read().decode(errors='replace')[-24000:]


def fetch_installer(url, destination, environment, workspace):
    return run(['curl', '--disable', '-fsSL', '--connect-timeout', '10',
                '--max-time', '45', '--max-filesize', str(2*1024*1024),
                url, '--output', str(destination)], environment, workspace, timeout=50)


class GitHubMetadataRedirect(urllib.request.HTTPRedirectHandler):
    """Keep the parent metadata credential on the HTTPS GitHub API only."""

    def redirect_request(self, request, fp, code, message, headers, new_url):
        target = urllib.parse.urlsplit(new_url)
        if target.scheme != 'https' or target.netloc.lower() != 'api.github.com':
            raise ValueError('metadata redirect outside GitHub API refused')
        return super().redirect_request(request, fp, code, message, headers, new_url)


def latest_release(repository):
    if not re.fullmatch(r'[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+', repository):
        raise ValueError('invalid GitHub repository identity')
    headers = {'Accept': 'application/vnd.github+json', 'User-Agent': 'public-install-census'}
    token = os.environ.get('CENSUS_GITHUB_METADATA_TOKEN', '')
    if token:
        if len(token) > 4096 or any(ord(char) < 33 or ord(char) > 126 for char in token):
            raise ValueError('metadata token has invalid header bytes')
        headers['Authorization'] = 'Bearer ' + token
    request = urllib.request.Request(
        'https://api.github.com/repos/' + repository + '/releases/latest', headers=headers)
    opener = urllib.request.build_opener(GitHubMetadataRedirect())
    with opener.open(request, timeout=30) as response:
        return json.load(response)['tag_name']


def probe(row, workspace):
    result = dict(repository=row['repository'], release_tag=row['release_tag'],
                  source_sha=row['source_sha'], documented_command=row['command'],
                  target=platform.platform(), machine=platform.machine())
    if not row['command']:
        return dict(result, outcome='MISSING_DOCUMENTED_ONE_LINER')
    match = re.fullmatch(r'curl -fsSL (https://raw\.githubusercontent\.com/[^ ]+/install\.sh) \| (sh|bash)', row['command'])
    if not match:
        return dict(result, outcome='UNKNOWN_COMMAND_FORMAT')
    home = workspace / 'home'
    home.mkdir()
    temporary = workspace / 'tmp'
    temporary.mkdir()
    paths = [str(home/'.local/bin'), str(home/'.cargo/bin'),
             str(Path(sys.executable).parent)]
    node = shutil.which('node')
    if node:
        paths.append(str(Path(node).parent))
    paths += ['/usr/bin', '/bin', '/usr/sbin', '/sbin']
    environment = dict(HOME=str(home), PATH=os.pathsep.join(paths), TMPDIR=str(temporary),
                       LANG='C.UTF-8', LC_ALL='C.UTF-8', PIP_NO_INPUT='1',
                       GIT_TERMINAL_PROMPT='0', PYTHONNOUSERSITE='1')
    # Bind inspection to fetched script bytes before allowing execution.
    downloaded = workspace/'install.sh'
    code, error = fetch_installer(match[1], downloaded, environment, workspace)
    if code:
        return dict(result, outcome='SCRIPT_FETCH_FAILED', fetch_exit=code,
                    error=error.replace(str(workspace), '<probe>'))
    script = downloaded.read_bytes()
    digest = hashlib.sha256(script).hexdigest()
    result['fetched_installer_sha256'] = digest
    if len(script) > 2 * 1024 * 1024 or digest != row['installer_sha256']:
        return dict(result, outcome='UNKNOWN_SCRIPT_CHANGED')
    try:
        result['observed_latest_tag'] = latest_release(row['repository'])
    except Exception as error:
        return dict(result, outcome='LATEST_RELEASE_UNKNOWN', error=str(error))
    if result['observed_latest_tag'] != row['release_tag']:
        return dict(result, outcome='RELEASE_CHANGED_REBIND_REQUIRED')
    # Pipe the inspected bytes unchanged to the documented interpreter. This is
    # deliberately reported as inspected-byte replay, not a second URL fetch.
    code, output = run(['/bin/bash', '-o', 'pipefail', '-c',
                        'cat "$1" | "$2"', 'install-probe', str(downloaded), match[2]],
                       environment, workspace)
    result.update(install_exit=code, install_output=output.replace(str(workspace), '<probe>'),
                  execution='documented URL fetched anonymously, exact inspected bytes piped unchanged to documented shell',
                  inspected_script_bytes=len(script))
    if code:
        return dict(result, outcome='INSTALL_TIMEOUT' if code == 124 else 'INSTALL_FAILED')
    versions = []
    for command in row['commands']:
        found = shutil.which(command, path=environment['PATH'])
        # macOS temporary roots may be aliases (/tmp -> /private/tmp).
        # Compare physical paths on both sides while retaining outside-HOME refusal.
        if not found or not Path(found).resolve().is_relative_to(home.resolve()):
            return dict(result, outcome='INSTALLED_COMMAND_MISSING_OR_SHADOWED', command=command)
        args = ['version'] if command == 'dp' else ['--version']
        rc, text = run([found, *args], environment, workspace, timeout=30)
        versions.append(dict(command=command, exit=rc, output=text.replace(str(workspace), '<probe>')))
        if rc:
            return dict(result, outcome='VERSION_COMMAND_FAILED', versions=versions)
        observed_version = release_version(text)
        if observed_version is None:
            return dict(result, outcome='VERSION_NOT_REPORTED', versions=versions)
        if observed_version != release_version(row['release_tag']):
            return dict(result, outcome='VERSION_MISMATCH', versions=versions)
    markers = []
    for marker in sorted(home.rglob('.release.json')):
        markers.append(dict(path=str(marker.relative_to(home)), value=json.loads(marker.read_text())))
    if row['kind'] == 'source-site':
        return dict(result, outcome='SOURCE_SITE_NOT_RELEASE_INSTALL', versions=versions)
    if row['kind'] == 'source':
        if not markers:
            return dict(result, outcome='SOURCE_IDENTITY_UNPROVEN')
        return dict(result, outcome='SOURCE_PACKAGE_INSTALLED_NO_CLI', markers=markers)
    return dict(result, outcome='VERSION_VERIFIED', versions=versions, markers=markers)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--manifest', type=Path, default=Path(__file__).with_name('public-install-census.json'))
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    if not os.environ.get('GITHUB_ACTIONS') == 'true':
        parser.error('execute installers only on the disposable GitHub-hosted job, not a resident host')
    rows = json.loads(args.manifest.read_text())['repositories']
    args.output.mkdir(parents=True, exist_ok=True)
    results = []
    for row in rows:
        started = time.monotonic()
        with tempfile.TemporaryDirectory(prefix='public-install-') as directory:
            try:
                result = probe(row, Path(directory))
            except Exception as error:
                result = dict(repository=row['repository'], outcome='PROBE_ERROR', error=str(error))
        result['elapsed_seconds'] = round(time.monotonic()-started, 2)
        results.append(result)
        print(json.dumps({k:result.get(k) for k in ['repository','outcome','elapsed_seconds']}), flush=True)
        (args.output/'results.json').write_text(json.dumps(results, indent=2)+'\n')
    # A complete baseline is an observation, never a claim all installers passed.
    summary = {name:sum(r['outcome']==name for r in results) for name in sorted({r['outcome'] for r in results})}
    (args.output/'summary.json').write_text(json.dumps(summary, indent=2)+'\n')
    print(json.dumps(summary), flush=True)
    return int(any(r['outcome']=='PROBE_ERROR' for r in results))


if __name__ == '__main__':
    raise SystemExit(main())
