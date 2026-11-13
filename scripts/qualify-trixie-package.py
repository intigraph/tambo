#!/usr/bin/env python3
"""One frozen, offline source-package attempt inside the disposable job.

# arming: workflow_dispatch; no host package installation in this script.
"""
import argparse
import csv
import hashlib
import json
import os
from pathlib import Path
import re
import shlex
import shutil
import signal
import subprocess
import tomllib

HERE = Path(__file__).resolve().parent


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def prepare(source, work):
    """Create a native qualification package without changing upstream inputs."""
    package = tomllib.loads((source / 'Cargo.toml').read_text())
    version = package['package']['version'] + '+qual1'
    if not re.fullmatch(r'[0-9]+(?:\.[0-9]+){2}\+qual1', version):
        raise ValueError('unexpected package version')
    bins = [row['name'] for row in package['bin']]
    if len(bins) != 2 or any(not re.fullmatch(r'[a-z][a-z0-9-]*', name) for name in bins):
        raise ValueError('both declared release binaries required')
    if work.exists():
        raise ValueError('build directory already exists')
    shutil.copytree(source, work, ignore=shutil.ignore_patterns('.git'))
    debian = work / 'debian'
    if debian.exists():
        raise ValueError('upstream packaging needs an explicit reconciliation')
    (debian / 'source').mkdir(parents=True)
    (debian / 'cargo-home').mkdir()
    (debian / 'source/format').write_text('3.0 (native)\n')
    (debian / 'cargo-home/config.toml').write_text(
        '[source.crates-io]\nreplace-with = "debian"\n'
        '[source.debian]\ndirectory = "/usr/share/cargo/registry"\n')
    (debian / 'control').write_text(
        'Source: tambo\nSection: utils\nPriority: optional\n'
        'Maintainer: Qualification Fixture <nobody@example.invalid>\n'
        'Build-Depends: debhelper-compat (= 13), cargo, rustc\n'
        'Standards-Version: 4.7.0\nRules-Requires-Root: no\n\n'
        'Package: tambo\nArchitecture: any\n'
        'Depends: ${shlibs:Depends}, ${misc:Depends}\n'
        'Description: disposable selected-feature build qualification\n'
        ' This package is a qualification fixture, not an archive submission.\n')
    (debian / 'changelog').write_text(
        f'tambo ({version}) unstable; urgency=medium\n\n'
        '  * Disposable frozen-index qualification; no publication.\n\n'
        ' -- Qualification Fixture <nobody@example.invalid>  Wed, 04 Nov 2026 11:20:00 +0000\n')
    license_text = (source / 'LICENSE').read_text()
    (debian / 'copyright').write_text(
        'Format: https://www.debian.org/doc/packaging-manuals/copyright-format/1.0/\n'
        'Source: https://github.com/intigraph/tambo\n\nFiles: *\n'
        'Copyright: See upstream LICENSE and NOTICE; dependency terms are in the closure report\n'
        'License: Apache-2.0\n' + '\n'.join(' ' + (line or '.') for line in license_text.splitlines()) + '\n')
    install = '\n'.join(
        '\tinstall -D -m755 target/release/' + shlex.quote(name)
        + ' debian/tambo/usr/bin/' + shlex.quote(name) for name in bins)
    smoke = '\n'.join('\ttarget/release/' + shlex.quote(name) + ' --version' for name in bins)
    rules = ('#!/usr/bin/make -f\n%:\n\tdh $@ --buildsystem=none\n'
             'override_dh_auto_build:\n'
             '\tCARGO_HOME=$(CURDIR)/debian/cargo-home CARGO_BUILD_JOBS=2 '
             'cargo build --offline --locked --release --bins\n'
             'override_dh_auto_test:\n' + smoke + '\n'
             'override_dh_auto_install:\n' + install + '\n')
    (debian / 'rules').write_text(rules)
    (debian / 'rules').chmod(0o755)
    return version


def run(command, log, cwd, *, timeout=600):
    with log.open('w') as output:
        child = subprocess.Popen(command, cwd=cwd, stdout=output, stderr=subprocess.STDOUT,
                                 start_new_session=True, env={**os.environ,
                                 'SOURCE_DATE_EPOCH': '1793791200'})
        try:
            return child.wait(timeout=timeout)
        except subprocess.TimeoutExpired:
            # Kill the command's group, including compiler grandchildren, before
            # cleanup can remove the build tree. Never retry a timed-out build.
            try:
                os.killpg(child.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
            child.wait(timeout=10)
            output.write('\nqualification command timed out; no automatic retry\n')
            return 124


def inventory(manifest, output):
    with (output / 'lift.csv').open('w', newline='') as handle:
        writer = csv.writer(handle)
        writer.writerow(['crate', 'locked_version', 'obligation', 'measured_lift'])
        for row in manifest['closure']['packages']:
            if not row['source']:
                continue
            obligation = ('name candidate absent' if not row['distro_name_candidates']
                          else 'locked version candidate absent' if not row['exact_locked_version_candidates']
                          else 'source/features/API need build qualification')
            writer.writerow([row['name'], row['version'], obligation,
                             'one named compatibility/source-package obligation; effort UNKNOWN'])
    actual = []
    for file in sorted(Path('/usr/share/cargo/registry').glob('*/Cargo.toml')):
        package = tomllib.loads(file.read_text())['package']
        files = []
        for item in file.parent.iterdir():
            if item.is_file() and re.match(r'(?i)^(license|copying|copyright|notice)([._-]|$)', item.name):
                files.append({'name': item.name, 'sha256': digest(item)})
        actual.append({key: package.get(key) for key in ('name', 'version', 'license', 'license-file')}
                      | {'license_files': files})
    (output / 'distro-registry-license-files.json').write_text(json.dumps(actual, indent=2))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--source', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    if os.environ.get('GITHUB_ACTIONS') != 'true' or not Path('/.dockerenv').is_file():
        raise ValueError('disposable hosted container required')
    source, output = args.source.resolve(), args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    manifest = json.loads((HERE / 'trixie-qualification.json').read_text())
    for name, expected in manifest['source_files'].items():
        if digest(source / name) != expected:
            raise ValueError('source input pin mismatch')
    work = source.parent / 'qualification-build' / 'tambo'
    version = prepare(source, work)
    lock = digest(work / 'Cargo.lock')
    inventory(manifest, output)
    run(['dpkg-query', '-W', '-f=${binary:Package}\t${Version}\n'], output / 'installed-packages.log', work)
    run(['rustc', '--version'], output / 'compiler.log', work)
    source_exit = run(['dpkg-source', '-b', '.'], output / 'source-package.log', work, timeout=120)
    build_exit = run(['dpkg-buildpackage', '-us', '-uc', '-b'], output / 'build.log', work)
    source_lint = run(['lintian', '--allow-root', '--display-info', '--pedantic', f'tambo_{version}.dsc'],
                      output / 'lintian-source.log', work.parent, timeout=120) if source_exit == 0 else None
    binaries = sorted(work.parent.glob('*.deb'))
    binary_lint = run(['lintian', '--allow-root', '--display-info', '--pedantic', *map(str, binaries)],
                      output / 'lintian-binary.log', work.parent, timeout=120) if binaries else None
    unchanged = digest(work / 'Cargo.lock') == lock
    receipt = dict(source_ref=manifest['source_ref'], snapshot=manifest['snapshot'],
                   release_features='default, both declared bins; no feature-dropping patch',
                   source_package_exit=source_exit, build_exit=build_exit,
                   lintian_source_exit=source_lint, lintian_binary_exit=binary_lint,
                   binary_count=len(binaries), lock_unchanged=unchanged,
                   package_smoke='both --version via dh_auto_test; no dev-feature unit-test claim',
                   artifacts=[{'name': p.name, 'sha256': digest(p)} for p in binaries],
                   limits='License inventory is not DFSG clearance; candidate counts are not compiler units')
    (output / 'result.json').write_text(json.dumps(receipt, indent=2))
    return 0 if source_exit == build_exit == source_lint == binary_lint == 0 and unchanged else 1


if __name__ == '__main__':
    raise SystemExit(main())
