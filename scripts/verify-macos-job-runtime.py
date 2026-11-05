#!/usr/bin/env python3
"""Unprivileged artifact verification; native imports do not establish UID readiness."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]


def verify(directory, native=False):
    if os.geteuid() == 0:
        raise ValueError('inspect this artifact unprivileged')
    expected = json.loads((ROOT / 'docs/operator-packets/e579/runtime/artifact.json').read_text())
    raw = (directory / 'manifest.json').read_bytes()
    if hashlib.sha256(raw).hexdigest() != expected['manifest_sha256']:
        raise ValueError('artifact manifest pin differs')
    manifest = json.loads(raw)
    files = manifest['files']
    actual = {str(p.relative_to(directory)) for p in directory.rglob('*') if p.is_file()}
    if (actual != set(files) | {'manifest.json'} or any(p.is_symlink() for p in directory.rglob('*'))
            or manifest['version'] != 1 or manifest['platform'] != 'macos'):
        raise ValueError('artifact exact inventory differs')
    for name, pin in files.items():
        path = directory / name
        # Names are fixed by the reviewed manifest pin; still reject traversal.
        if any(part in ('', '.', '..') for part in name.split('/')) or name.startswith('/'):
            raise ValueError('unsafe runtime path')
        digest = hashlib.sha256()
        with path.open('rb') as stream:
            for chunk in iter(lambda: stream.read(1048576), b''):
                digest.update(chunk)
        if digest.hexdigest() != pin:
            raise ValueError('artifact member digest differs')
        if path.stat().st_mode & 0o222:
            raise ValueError('artifact member is writable')
    if native:
        if sys.platform != 'darwin' or platform.machine() != 'arm64':
            raise ValueError('native arm64 macOS import proof required')
        runtime = directory.resolve() / 'runtime'
        code = ('import sys; from pathlib import Path; '
                'assert Path(sys.prefix)==Path(sys.argv[1]); '
                'assert sys.flags.isolated and sys.flags.no_site and sys.dont_write_bytecode; '
                'sys.path.insert(0,str(Path(sys.argv[1])/"app")); '
                'from shantytown import job_launch,job_metadata,actor_identity,ephemeral; from shantytown.cli import build_parser; build_parser(); '
                'print("NATIVE private prefix and isolated application imports PASS; readiness UNPROVED")')
        subprocess.run([str(runtime / 'bin/python3'), '-BIS', '-c', code, str(runtime)],
                       env={'PATH': '/usr/bin:/bin', 'LC_ALL': 'C'}, check=True, timeout=30)
    print('VERIFIED pinned runtime inventory; native=' + str(native) + '; readiness UNPROVED')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--directory', type=Path, required=True)
    parser.add_argument('--native', action='store_true')
    args = parser.parse_args()
    verify(args.directory, args.native)
