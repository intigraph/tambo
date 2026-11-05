#!/usr/bin/env python3
"""Build a deterministic, credential-free runtime from reviewed local archives.

No network, native execution, root build or runtime installation is performed.
Pinned archive links are materialized as regular files; output has no symlinks.
"""
import argparse
import gzip
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import re
import struct
import tarfile

ROOT = Path(__file__).resolve().parents[1]
SOURCES = ROOT / 'docs/operator-packets/e579/runtime/sources.json'
MAX_FILE = 256 * 1024 * 1024
MAX_TOTAL = 1024 * 1024 * 1024


def digest(path):
    result = hashlib.sha256()
    with path.open('rb') as stream:
        for chunk in iter(lambda: stream.read(1048576), b''):
            result.update(chunk)
    return result.hexdigest()


def safe(name):
    return bool(name) and all(part not in ('', '.', '..') for part in name.split('/')) and bool(re.fullmatch('[A-Za-z0-9/._-]+', name))


def archive(path, expected):
    if path.is_symlink() or not path.is_file() or digest(path) != expected:
        raise ValueError('reviewed archive digest mismatch')
    value = tarfile.open(path, 'r:gz')
    members = value.getmembers()
    if len(members) > 16384 or len({m.name for m in members}) != len(members):
        value.close()
        raise ValueError('archive duplicate or unbounded inventory')
    for member in members:
        if (not safe(member.name.rstrip('/')) or member.size > MAX_FILE
                or not (member.isfile() or member.isdir() or member.issym() or member.islnk())):
            value.close()
            raise ValueError('unsafe archive member')
    return value


def member_bytes(source, name):
    member = source.getmember(name)
    seen = set()
    while member.issym() or member.islnk():
        if member.name in seen or len(seen) >= 16:
            raise ValueError('archive link cycle')
        seen.add(member.name)
        target = (PurePosixPath(member.name).parent / member.linkname if member.issym()
                  else PurePosixPath(member.linkname))
        # Resolve only inside the pinned archive, never on the host filesystem.
        parts = []
        if target.is_absolute():
            raise ValueError('absolute archive link')
        for part in target.parts:
            if part == '..':
                if not parts:
                    raise ValueError('escaping archive link')
                parts.pop()
            elif part != '.':
                parts.append(part)
        member = source.getmember('/'.join(parts))
    if not member.isfile() or member.size > MAX_FILE:
        raise ValueError('bounded regular archive member required')
    with source.extractfile(member) as stream:
        raw = stream.read(MAX_FILE + 1)
    if len(raw) != member.size or len(raw) > MAX_FILE:
        raise ValueError('archive member changed or exceeded bound')
    return raw


def arm64(raw):
    # Native little-endian Mach-O64 with CPU_TYPE_ARM64, not a venv/link/script.
    if len(raw) < 8 or struct.unpack('<II', raw[:8]) != (0xfeedfacf, 0x0100000c):
        raise ValueError('native arm64 Mach-O interpreter/client required')


def build(python_archive, app_archive, seeds_archive, output, sources=SOURCES):
    if os.geteuid() == 0:
        raise ValueError('build as unprivileged user; never build or download as root')
    pins = json.loads(sources.read_text())
    inputs = [(python_archive, pins['python']['sha256']), (app_archive, pins['shantytown']['sha256']),
              (seeds_archive, pins['seeds']['sha256'])]
    # Verify all inputs before creating an output. Never extractall foreign paths.
    archives = []
    try:
        for path, pin in inputs:
            archives.append(archive(path, pin))
        py, app, sd = archives
        files = {}
        files['runtime/bin/python3'] = member_bytes(py, 'python/bin/python3')
        arm64(files['runtime/bin/python3'])
        for member in py.getmembers():
            name = member.name
            if not name.startswith('python/lib/') or member.isdir():
                continue
            relative = name.removeprefix('python/')
            # No pip/site hooks, bytecode, tests or developer config in a root runtime.
            if any(part in {'site-packages', '__pycache__', 'test', 'tests'} for part in PurePosixPath(relative).parts):
                continue
            files['runtime/' + relative] = member_bytes(py, name)
        prefix = 'shantytown-' + pins['shantytown']['commit'] + '/'
        for member in app.getmembers():
            if member.isfile() and member.name.startswith(prefix + 'shantytown/'):
                relative = member.name.removeprefix(prefix)
                if any(part in {'__pycache__', 'tests', '.git'} for part in PurePosixPath(relative).parts):
                    continue
                files['runtime/app/' + relative] = member_bytes(app, member.name)
        files['launcher'] = member_bytes(app, prefix + 'scripts/st-job-launcher')
        sd_name = 'sd-seeds-ai-v' + pins['seeds']['version'] + '-aarch64-apple-darwin/sd'
        files['runtime/bin/sd'] = member_bytes(sd, sd_name)
        arm64(files['runtime/bin/sd'])
        # Keep shipped licenses as data in an allowed library subtree.
        files['runtime/lib/licenses/shantytown-LICENSE'] = member_bytes(app, prefix + 'LICENSE')
        files['runtime/lib/licenses/seeds-LICENSE'] = member_bytes(sd, str(PurePosixPath(sd_name).parent / 'LICENSE'))
        if (not files['launcher'].startswith(b'#!/opt/shantytown-job-launcher/bin/python3 -BIS\n')
                or len(files) > 4096 or sum(map(len, files.values())) > MAX_TOTAL
                or any(not safe(name) for name in files)):
            raise ValueError('runtime inventory or launcher outside delivery contract')
        for required in ['runtime/app/shantytown/job_launch.py', 'runtime/app/shantytown/job_metadata.py',
                         'runtime/lib/python3.12/os.py', 'runtime/lib/python3.12/encodings/__init__.py']:
            if required not in files:
                raise ValueError('incomplete runtime')
        output.mkdir(mode=0o755)  # Exclusive; partial output is retained on later failure.
        manifest = {'version': 1, 'platform': 'macos', 'files': {name: hashlib.sha256(raw).hexdigest()
                                                               for name, raw in sorted(files.items())}}
        for name, raw in sorted(files.items()):
            path = output / name
            path.parent.mkdir(parents=True, exist_ok=True)
            with path.open('xb') as stream:
                stream.write(raw)
            path.chmod(0o555 if name == 'launcher' or name.startswith('runtime/bin/') else 0o444)
        manifest_path = output / 'manifest.json'
        manifest_path.write_text(json.dumps(manifest, sort_keys=True, indent=2) + '\n')
        manifest_path.chmod(0o444)
        pin = digest(manifest_path)
        for path in sorted(output.rglob('*'), reverse=True):
            if path.is_dir():
                path.chmod(0o555)
        output.chmod(0o555)
        return pin
    finally:
        for source in archives:
            source.close()


def pack(directory, target):
    # Stable mtimes/owners/modes/order and gzip header; no local path in the tar.
    with target.open('xb') as stream, gzip.GzipFile(fileobj=stream, mode='wb', filename='', mtime=0) as zipped:
        with tarfile.open(fileobj=zipped, mode='w', format=tarfile.PAX_FORMAT) as output:
            for path in [directory, *sorted(directory.rglob('*'))]:
                name = directory.name + ('/' + str(path.relative_to(directory)) if path != directory else '')
                info = output.gettarinfo(str(path), arcname=name)
                info.uid = info.gid = info.mtime = 0
                info.uname = info.gname = ''
                if path.is_file():
                    with path.open('rb') as source:
                        output.addfile(info, source)
                else:
                    output.addfile(info)
    target.with_name(target.name + '.sha256').write_text(digest(target) + '  ' + target.name + '\n')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--python-archive', type=Path, required=True)
    parser.add_argument('--shantytown-archive', type=Path, required=True)
    parser.add_argument('--seeds-archive', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    pin = build(args.python_archive, args.shantytown_archive, args.seeds_archive, args.output)
    target = args.output.with_name(args.output.name + '.tar.gz')
    pack(args.output, target)
    print(json.dumps({'manifest_sha256': pin, 'archive': target.name, 'archive_sha256': digest(target)}))


if __name__ == '__main__':
    main()
