#!/usr/bin/env python3
"""Steve-run account-only packet. Never installs sudo policy, keys or job runtime."""
import argparse
import fcntl
import hashlib
import json
import os
from pathlib import Path
import re
import stat
import subprocess
import sys

BASE = Path('/Library/Application Support/Shantytown')
JOBS = BASE / 'jobs'
TOOL = BASE / 'bin/tambo-e579'
STOP = JOBS / 'account-packet-rolled-back'


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


def run(args, ok=True):
    result = subprocess.run(args, stdin=subprocess.DEVNULL, capture_output=True,
                            text=True, timeout=15,
                            env={'PATH': '/usr/bin:/bin:/usr/sbin:/sbin', 'LC_ALL': 'C'})
    if ok and result.returncode:
        raise ValueError('native command refused: ' + args[0])
    return result


def inputs(tool, tool_pin, inventory, inventory_pin):
    raw = regular(tool)
    if digest(raw) != tool_pin:
        raise ValueError('tool digest mismatch')
    catalog = regular(inventory / 'slots.json', 1048576)
    if digest(catalog) != inventory_pin:
        raise ValueError('catalog digest mismatch')
    parsed = json.loads(catalog)
    slots = parsed['slots']
    if parsed['version'] != 1 or not 1 <= len(slots) <= 1024:
        raise ValueError('bounded version1 slot inventory required')
    files = {'slots.json': catalog}
    # The native provisioner validates the entire manifest schema and UID range.
    # Validate names and pins before constructing any paths or writing files.
    for principal, row in slots.items():
        if not re.fullmatch(r'eph-(review|merge|triage|task)-[0-9a-f]{6}', principal):
            raise ValueError('invalid principal')
        value = regular(inventory / (principal + '.json'), 65536)
        if digest(value) != row['sha256']:
            raise ValueError('slot digest mismatch')
        files[principal + '.json'] = value
    return raw, files


def plan():
    protected(TOOL)
    protected(JOBS / 'slots.json')
    value = json.loads(run([str(TOOL), 'job-slots', '--directory', str(JOBS)]).stdout)
    for row in value['accounts']:
        protected(JOBS / (row['principal'] + '.json'))
    return value['accounts']


def properties(record, keys):
    result = run(['/usr/bin/dscl', '.', '-read', record, *keys])
    values = {}
    for line in result.stdout.splitlines():
        key, value = line.split(':', 1)
        if key in values:
            raise ValueError('duplicate native property')
        values[key] = value.strip()
    return values


def verify(accounts):
    users = run(['/usr/bin/dscl', '.', '-list', '/Users', 'UniqueID']).stdout
    groups = run(['/usr/bin/dscl', '.', '-list', '/Groups', 'PrimaryGroupID']).stdout
    for row in accounts:
        uid, name = row['uid'], row['username']
        expected = {'UniqueID': str(uid), 'PrimaryGroupID': str(uid),
                    'NFSHomeDirectory': row['home'], 'UserShell': '/usr/bin/false',
                    'IsHidden': '1', 'AuthenticationAuthority': ';DisabledUser;'}
        if properties('/Users/' + name, list(expected)) != expected:
            raise ValueError('account identity/login settings differ')
        for listing in (users, groups):
            matches = [line.split() for line in listing.splitlines()
                       if line.split() and line.split()[-1] == str(uid)]
            if matches != [[name, str(uid)]]:
                raise ValueError('native UID/GID is not unique')
        member = run(['/usr/sbin/dseditgroup', '-o', 'checkmember', '-m', name, 'admin'], ok=False)
        if member.returncode == 0 or member.stdout.strip() != 'no ' + name + ' is NOT a member of admin':
            raise ValueError('non-admin membership not proved')
        home = Path('/private' + row['home'])
        protected(home.parent)
        info = home.lstat()
        if not stat.S_ISDIR(info.st_mode) or info.st_uid != uid or info.st_gid != uid or stat.S_IMODE(info.st_mode) != 0o700:
            raise ValueError('home ownership/mode differs')
        receipt = JOBS / 'account-receipts' / (str(uid) + '.json')
        protected(receipt)
        if json.loads(regular(receipt, 4096)) != {
            'principal': row['principal'], 'uid': uid,
            'sha256': row['manifest_sha256'], 'state': 'complete'
        }:
            raise ValueError('complete nonreuse receipt not proved')
    print('VERIFIED disabled-login accounts, unique UIDs/GIDs, private homes and durable receipts')


def execute(args):
    if sys.platform != 'darwin' or os.geteuid() != 0:
        raise ValueError('Steve must run this with sudo on macOS')
    os.umask(0o077)
    tool_raw = files = None
    if args.mode == 'setup':
        tool_raw, files = inputs(args.tool, args.tool_sha256, args.inventory, args.inventory_sha256)
        # Validate source using the pinned executable only after its protected copy
        # exists. No account operation precedes native whole-plan validation.
        for path in (BASE, BASE / 'bin', JOBS):
            directory(path)
        install(TOOL, tool_raw, 0o755)
    else:
        protected(JOBS)
    lock = JOBS / '.account-packet-lock'
    fd = os.open(lock, os.O_RDWR | os.O_CREAT | os.O_NOFOLLOW, 0o600)
    try:
        protected(lock)
        if stat.S_IMODE(os.fstat(fd).st_mode) != 0o600:
            raise ValueError('packet lock mode differs')
        fcntl.flock(fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
        if args.mode == 'setup':
            if STOP.exists() or STOP.is_symlink():
                raise ValueError('rollback tombstone retained; explicit new packet required')
            for name, raw in files.items():
                install(JOBS / name, raw, 0o644)
            accounts = plan()
            run([str(TOOL), 'job-slots', '--directory', str(JOBS), '--apply'])
            verify(accounts)
        elif args.mode == 'verify':
            verify(plan())
            print('rollback tombstone: ' + str(STOP.exists()))
        else:
            accounts = plan()
            verify(accounts)  # Refuse partial/drifted state; never guess a recovery.
            # Provisioning created disabled-login accounts only. No jobs, sudo
            # policy, registry, credentials or readiness were installed to undo.
            # Preserve accounts/homes/receipts: deletion would recycle identities.
            install(STOP, b'e579 account-only provisioning rolled back; no reapply\n', 0o600)
            print('ROLLED BACK account packet admission; identities/data/receipts retained')
    finally:
        os.close(fd)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('mode', choices=['setup', 'verify', 'rollback'])
    parser.add_argument('--tool', type=Path)
    parser.add_argument('--tool-sha256')
    parser.add_argument('--inventory', type=Path)
    parser.add_argument('--inventory-sha256')
    args = parser.parse_args()
    if args.mode == 'setup' and (args.tool is None or args.inventory is None or any(
            not re.fullmatch('[0-9a-f]{64}', pin or '')
            for pin in (args.tool_sha256, args.inventory_sha256))):
        parser.error('setup requires reviewed tool/inventory paths and both SHA256 pins')
    try:
        execute(args)
    except (ValueError, OSError, KeyError, TypeError, subprocess.SubprocessError) as error:
        print('REFUSED: ' + str(error), file=sys.stderr)
        return 1
    return 0


if __name__ == '__main__':
    sys.exit(main())
