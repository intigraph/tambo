#!/usr/bin/env python3
"""Operator-run macOS controller account packet; no keys, policy or job activation."""
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
import tempfile

BASE = Path('/Library/Application Support/Shantytown')
STATE = BASE / 'controllers'
RECEIPT = STATE / '51000.json'
STOP = STATE / 'controller-packet-rolled-back'
TOOL = BASE / 'bin/tambo-controller-e579'


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


def run(args, ok=True):
    result = subprocess.run(args, stdin=subprocess.DEVNULL, capture_output=True,
                            text=True, timeout=15,
                            env={'PATH': '/usr/bin:/bin:/usr/sbin:/sbin', 'LC_ALL': 'C'})
    if ok and result.returncode:
        raise ValueError('native command refused: ' + args[0])
    return result


def properties(record, keys):
    result = run(['/usr/bin/dscl', '.', '-read', record, *keys])
    values = {}
    for line in result.stdout.splitlines():
        key, value = line.removeprefix('dsAttrTypeNative:').split(':', 1)
        if key in values:
            raise ValueError('duplicate native property')
        values[key] = value.strip()
    return values


def packet(path, pin):
    raw = regular(path, 16384)
    if not re.fullmatch('[0-9a-f]{64}', pin or '') or digest(raw) != pin:
        raise ValueError('explicit controller packet digest mismatch')
    row = json.loads(raw)
    if (not isinstance(row, dict) or set(row) != {'version', 'platform', 'uid', 'gid', 'username', 'home'}
            or type(row['version']) is not int or row['version'] != 1 or row['platform'] != 'macos'
            or type(row['uid']) is not int or row['uid'] < 10000
            or row['uid'] != 51000 or type(row['gid']) is not int or row['gid'] != 51000
            or row['username'] != '_stctl'
            or row['home'] != '/var/lib/shantytown/controllers/_stctl'):
        raise ValueError('exact dedicated controller UID/GID51000 packet required')
    return row, pin


def catalog(directory, pin, uid):
    raw = regular(directory / 'slots.json', 1048576)
    if not re.fullmatch('[0-9a-f]{64}', pin or '') or digest(raw) != pin:
        raise ValueError('explicit catalog digest mismatch')
    data = json.loads(raw)
    if data.get('version') != 1 or set(data.get('slots', {})) != {'eph-task-000001'}:
        raise ValueError('exact initial slot catalog required')
    name = 'eph-task-000001'
    member = regular(directory / (name + '.json'), 65536)
    if digest(member) != data['slots'][name]['sha256']:
        raise ValueError('catalog member digest mismatch')
    spec = json.loads(member)
    if (spec.get('principal') != name or spec.get('controller_uid') != uid
            or spec.get('uid') != 51001 or spec.get('gid') != 51001):
        raise ValueError('controller UID differs from pinned job manifest')
    return {'slots.json': raw, name + '.json': member}


def inventory(kind, key):
    answer = run(['/usr/bin/dscl', '.', '-list', kind, key]).stdout
    result = {}
    for line in answer.splitlines():
        parts = line.split()
        if len(parts) != 2 or not parts[1].isdigit() or parts[0] in result:
            raise ValueError('native account inventory unknown')
        result[parts[0]] = int(parts[1])
    if not result:
        raise ValueError('native account inventory empty')
    return result


def inventories():
    return inventory('/Users', 'UniqueID'), inventory('/Groups', 'PrimaryGroupID')


def unique(row, users, groups):
    for table in (users, groups):
        if table.get(row['username']) != row['uid'] or list(table.values()).count(row['uid']) != 1:
            raise ValueError('controller UID/GID is not unique')


def native_verify(row):
    unique(row, *inventories())
    name = row['username']
    expected = {'UniqueID': str(row['uid']), 'PrimaryGroupID': str(row['gid']),
                'NFSHomeDirectory': row['home'], 'UserShell': '/usr/bin/false',
                'IsHidden': '1', 'AuthenticationAuthority': ';DisabledUser;'}
    if properties('/Users/' + name, list(expected)) != expected:
        raise ValueError('controller native identity/login settings differ')
    member = run(['/usr/sbin/dseditgroup', '-o', 'checkmember', '-m', name, 'admin'], ok=False)
    if member.returncode == 0 or member.stdout.strip() != 'no ' + name + ' is NOT a member of admin':
        raise ValueError('controller non-admin membership not proved')
    own_groups = run(['/usr/bin/id', '-G', name]).stdout.split()
    users, _ = inventories()
    login = [user for user, uid in users.items() if uid == 501]
    if len(login) != 1 or not own_groups or any(not value.isdigit() for value in own_groups):
        raise ValueError('controller/login group inventory unknown')
    login_groups = run(['/usr/bin/id', '-G', login[0]]).stdout.split()
    if (set(own_groups) != {str(row['gid'])} or not login_groups
            or any(not value.isdigit() for value in login_groups)
            or set(own_groups).intersection(login_groups)):
        raise ValueError('controller shares a login group')
    home = Path('/private' + row['home'])
    protected(home.parent)
    info = home.lstat()
    if (not stat.S_ISDIR(info.st_mode) or info.st_uid != row['uid'] or info.st_gid != row['gid']
            or stat.S_IMODE(info.st_mode) != 0o700):
        raise ValueError('controller private home differs')


def receipt(row, pin, state):
    return {'version': 1, 'uid': row['uid'], 'packet_sha256': pin, 'state': state}


def read_receipt():
    protected(RECEIPT)
    if stat.S_IMODE(RECEIPT.lstat().st_mode) != 0o600:
        raise ValueError('controller receipt mode differs')
    return json.loads(regular(RECEIPT, 4096))


def save_complete(row, pin):
    # Only our exact prepared reservation can transition; preserve any failure.
    if read_receipt() != receipt(row, pin, 'prepared'):
        raise ValueError('controller reservation changed')
    fd, name = tempfile.mkstemp(prefix='.complete-', dir=STATE)
    with os.fdopen(fd, 'wb') as stream:
        os.fchmod(stream.fileno(), 0o600)
        stream.write(json.dumps(receipt(row, pin, 'complete'), sort_keys=True).encode() + b'\n')
        stream.flush()
        os.fsync(stream.fileno())
    os.replace(name, RECEIPT)
    fd = os.open(STATE, os.O_RDONLY)
    try:
        os.fsync(fd)
    finally:
        os.close(fd)


def setup(row, pin, tool_raw=None, catalog_files=None):
    if STOP.exists() or STOP.is_symlink():
        raise ValueError('rollback tombstone retained; no automatic reapply')
    if RECEIPT.exists() or RECEIPT.is_symlink():
        if read_receipt() != receipt(row, pin, 'complete'):
            raise ValueError('prepared/nonreuse reservation requires explicit recovery')
        protected(TOOL)
        if tool_raw is None or regular(TOOL) != tool_raw:
            raise ValueError('installed controller tool differs; explicit pinned replacement required')
        native_verify(row)
        return 'UNCHANGED controller account verified'
    users, groups = inventories()
    home = Path('/private' + row['home'])
    if (any(row['username'] in table or row['uid'] in table.values() for table in (users, groups))
            or home.exists() or home.is_symlink()):
        raise ValueError('controller account/UID/GID/home collision; no adoption')
    # Collision preflight precedes native mutation. Reserve before the first dscl.
    for path in (BASE, STATE):
        directory(path)
    directory(BASE / 'bin')
    install(TOOL, tool_raw, 0o755)
    preview = STATE / 'catalog'
    directory(preview)
    for name, raw in (catalog_files or {}).items():
        install(preview / name, raw, 0o644)
    planned = json.loads(run([str(TOOL), 'job-slots', '--directory', str(preview)]).stdout)
    if (len(planned.get('accounts', [])) != 1
            or planned['accounts'][0].get('controller_uid') != row['uid']
            or planned['accounts'][0].get('uid') != 51001):
        raise ValueError('native whole-plan controller binding differs')
    install(RECEIPT, json.dumps(receipt(row, pin, 'prepared'), sort_keys=True).encode() + b'\n', 0o600)
    for path in (Path('/private/var'), Path('/private/var/lib'),
                 Path('/private/var/lib/shantytown'), home.parent):
        directory(path)
    name = row['username']
    for kind, values in [('/Groups/', {'PrimaryGroupID': str(row['gid']), 'Password': '*'}),
                         ('/Users/', {'UniqueID': str(row['uid']), 'PrimaryGroupID': str(row['gid']),
                                      'NFSHomeDirectory': row['home'], 'UserShell': '/usr/bin/false',
                                      'Password': '*', 'IsHidden': '1', 'AuthenticationAuthority': ';DisabledUser;'})]:
        run(['/usr/bin/dscl', '.', '-create', kind + name])
        for key, value in values.items():
            run(['/usr/bin/dscl', '.', '-create', kind + name, key, value])
    home.mkdir(mode=0o700)
    home.chmod(0o700)
    os.chown(home, row['uid'], row['gid'])
    native_verify(row)
    save_complete(row, pin)
    return 'CREATED disabled-login controller; no key/policy/readiness grant'


def execute(args):
    row, pin = packet(args.packet, args.packet_sha256)
    catalog_files = catalog(args.inventory, args.inventory_sha256, row['uid'])
    tool_raw = None
    if args.mode == 'setup':
        if args.tool is None:
            raise ValueError('explicit reviewed tool required')
        tool_raw = regular(args.tool)
        if not re.fullmatch('[0-9a-f]{64}', args.tool_sha256 or '') or digest(tool_raw) != args.tool_sha256:
            raise ValueError('explicit tool digest mismatch')
    if args.offline:
        if args.mode != 'verify':
            raise ValueError('offline is input verification only')
        return 'OFFLINE packet input verified; native controller/readiness UNPROVED'
    if sys.platform != 'darwin' or os.geteuid() != 0:
        raise ValueError('operator sudo on native macOS required')
    if args.mode != 'setup':
        protected(TOOL)
        if not re.fullmatch('[0-9a-f]{64}', args.tool_sha256 or '') or digest(regular(TOOL)) != args.tool_sha256:
            raise ValueError('installed tool digest mismatch')
    if args.mode == 'verify':
        if read_receipt() != receipt(row, pin, 'complete'):
            raise ValueError('complete controller reservation not proved')
        native_verify(row)
        return 'VERIFIED native controller; rollback tombstone=' + str(STOP.exists())
    os.umask(0o077)
    # Existing root-owned base from the account packet is required; no writes to
    # job manifests, account receipts, identity policy, keys or readiness.
    protected(BASE)
    lock = BASE / '.controller-packet-lock'
    fd = os.open(lock, os.O_RDWR | os.O_CREAT | os.O_NOFOLLOW, 0o600)
    try:
        protected(lock)
        if stat.S_IMODE(os.fstat(fd).st_mode) != 0o600:
            raise ValueError('controller lock mode differs')
        fcntl.flock(fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
        if args.mode == 'setup':
            return setup(row, pin, tool_raw, catalog_files)
        if read_receipt() != receipt(row, pin, 'complete'):
            raise ValueError('complete controller reservation not proved')
        native_verify(row)
        install(STOP, b'controller account-only admission rolled back; preserve UID/home/receipt\n', 0o600)
        return 'ROLLED BACK account packet; identities/data retained, later grants need separate rollback'
    finally:
        os.close(fd)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('mode', choices=['setup', 'verify', 'rollback'])
    parser.add_argument('--packet', type=Path, required=True)
    parser.add_argument('--packet-sha256', required=True)
    parser.add_argument('--inventory', type=Path, required=True)
    parser.add_argument('--inventory-sha256', required=True)
    parser.add_argument('--tool', type=Path)
    parser.add_argument('--tool-sha256')
    parser.add_argument('--offline', action='store_true', help='verify input only; no native readiness claim')
    args = parser.parse_args()
    try:
        print(execute(args))
        return 0
    except (ValueError, OSError, KeyError, TypeError) as error:
        print('REFUSED: ' + str(error), file=sys.stderr)
        return 1


if __name__ == '__main__':
    raise SystemExit(main())
