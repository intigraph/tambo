#!/usr/bin/env python3
"""Operator-only create-only staging of a reviewed runtime archive; never applies it."""
import argparse
import hashlib
import io
import json
import os
from pathlib import Path
import re
import stat
import sys
import tarfile

BASE = Path('/Library/Application Support/Shantytown/runtime-artifacts')


def digest(raw):
    return hashlib.sha256(raw).hexdigest()


def regular(path, limit=268435456):
    fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW)
    with os.fdopen(fd, 'rb') as stream:
        info = os.fstat(stream.fileno())
        if not stat.S_ISREG(info.st_mode) or info.st_size > limit:
            raise ValueError('bounded regular file required: ' + str(path))
        raw = stream.read(limit + 1)
    if len(raw) > limit:
        raise ValueError('file grew beyond bound')
    return raw


def protected(path):
    for entry in (path, *path.parents):
        info = entry.lstat()
        if info.st_uid != 0 or info.st_mode & 0o022 or not (
            stat.S_ISREG(info.st_mode) or stat.S_ISDIR(info.st_mode)
        ):
            raise ValueError('root-owned, non-writable, non-symlink path required: ' + str(entry))


def directory(path):
    if not path.exists() and not path.is_symlink():
        protected(path.parent)
        path.mkdir(mode=0o755)
        path.chmod(0o755)
    protected(path)
    if not path.is_dir():
        raise ValueError('directory required')


def install(path, raw, mode):
    """Create-only, or exact unchanged. Never repair/adopt an occupied path."""
    protected(path.parent)
    if path.exists() or path.is_symlink():
        protected(path)
        if regular(path) != raw or stat.S_IMODE(path.lstat().st_mode) != mode:
            raise ValueError('occupied destination differs; retain it: ' + str(path))
        return
    fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, mode)
    with os.fdopen(fd, 'wb') as stream:
        os.fchmod(stream.fileno(), mode)
        stream.write(raw)
        stream.flush()
        os.fsync(stream.fileno())
    fd = os.open(path.parent, os.O_RDONLY)
    try:
        os.fsync(fd)
    finally:
        os.close(fd)


def read_archive(path, archive_pin, manifest_pin):
    if not all(re.fullmatch('[0-9a-f]{64}', value or '') for value in (archive_pin, manifest_pin)):
        raise ValueError('explicit archive and manifest pins required')
    raw = regular(path)
    if digest(raw) != archive_pin:
        raise ValueError('archive digest mismatch')
    with tarfile.open(fileobj=io.BytesIO(raw), mode='r:gz') as archive:
        members = archive.getmembers()
        if len(members) > 8192 or len({m.name for m in members}) != len(members):
            raise ValueError('unbounded or duplicate artifact inventory')
        files, dirs = {}, set()
        total = 0
        for member in members:
            prefix, slash, name = member.name.partition('/')
            if prefix != 'macos-arm64-job-runtime':
                raise ValueError('unexpected runtime archive root')
            if not slash and member.isdir():
                continue
            if (not slash or not name or not re.fullmatch('[A-Za-z0-9/._-]+', name)
                    or any(part in ('', '.', '..') for part in name.split('/'))):
                raise ValueError('unsafe runtime archive path')
            if member.isdir():
                dirs.add(name)
            elif member.isfile() and member.size <= 268435456:
                total += member.size
                if total > 1073741824:
                    raise ValueError('runtime exceeds byte bound')
                with archive.extractfile(member) as stream:
                    files[name] = stream.read(member.size + 1)
                if len(files[name]) != member.size:
                    raise ValueError('runtime member length differs')
            else:
                raise ValueError('runtime links/special files refused')
        manifest_raw = files.pop('manifest.json')
        if digest(manifest_raw) != manifest_pin:
            raise ValueError('runtime manifest pin differs')
        manifest = json.loads(manifest_raw)
        if (set(manifest) != {'version', 'platform', 'files'} or manifest['version'] != 1
                or manifest['platform'] != 'macos' or set(manifest['files']) != set(files)
                or len(files) > 4096):
            raise ValueError('runtime exact manifest inventory differs')
        expected_dirs = {str(parent) for name in set(files) | {'manifest.json'}
                         for parent in Path(name).parents if str(parent) != '.'}
        if dirs != expected_dirs:
            raise ValueError('runtime extra/missing directories')
        for name, raw in files.items():
            if digest(raw) != manifest['files'][name]:
                raise ValueError('runtime member digest differs')
        files['manifest.json'] = manifest_raw
        return files


def stage(args):
    if sys.platform != 'darwin' or os.geteuid() != 0:
        raise ValueError('operator sudo on native macOS required')
    files = read_archive(args.archive, args.archive_sha256, args.manifest_sha256)
    directory(BASE.parent)
    directory(BASE)
    destination = BASE / args.manifest_sha256
    if destination.exists() or destination.is_symlink():
        protected(destination)
        expected_dirs = {str(parent) for name in files for parent in Path(name).parents if str(parent) != '.'}
        actual_dirs = {str(p.relative_to(destination)) for p in destination.rglob('*') if p.is_dir()}
        if ({str(p.relative_to(destination)) for p in destination.rglob('*') if p.is_file()} != set(files)
                or actual_dirs != expected_dirs or any(p.is_symlink() for p in destination.rglob('*'))):
            raise ValueError('occupied runtime differs; retain it')
        for name, raw in files.items():
            path = destination / name
            protected(path)
            mode = 0o555 if name == 'launcher' or name.startswith('runtime/bin/') else 0o444
            if regular(path) != raw or stat.S_IMODE(path.lstat().st_mode) != mode:
                raise ValueError('occupied runtime bytes differ; retain it')
        return 'UNCHANGED protected artifact ' + str(destination)
    pending = BASE / ('.' + args.manifest_sha256 + '.pending')
    pending.mkdir(mode=0o755)  # Exclusive; partial staging is never removed automatically.
    pending.chmod(0o755)
    os.chown(pending, 0, 0)
    for name, raw in sorted(files.items()):
        path = pending / name
        for parent in reversed(path.parents):
            if parent == pending or parent.is_relative_to(pending):
                directory(parent)
                os.chown(parent, 0, 0)
        install(path, raw, 0o555 if name == 'launcher' or name.startswith('runtime/bin/') else 0o444)
        os.chown(path, 0, 0)
    for path in sorted(pending.rglob('*'), reverse=True):
        if path.is_dir():
            fd = os.open(path, os.O_RDONLY)
            try:
                os.fsync(fd)
            finally:
                os.close(fd)
    fd = os.open(pending, os.O_RDONLY)
    try:
        os.fsync(fd)
    finally:
        os.close(fd)
    os.rename(pending, destination)
    fd = os.open(BASE, os.O_RDONLY)
    try:
        os.fsync(fd)
    finally:
        os.close(fd)
    return 'STAGED protected artifact ' + str(destination) + '; no runtime apply or readiness grant'


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--archive', type=Path, required=True)
    parser.add_argument('--archive-sha256', required=True)
    parser.add_argument('--manifest-sha256', required=True)
    args = parser.parse_args()
    try:
        print(stage(args))
    except (ValueError, OSError, KeyError, TypeError) as error:
        print('REFUSED: ' + str(error), file=sys.stderr)
        raise SystemExit(1)
