#!/usr/bin/env python3
"""Explicit, operator-only expansion of the never-launched initial macOS slot."""
import argparse
from datetime import datetime, timezone
import fcntl
import importlib.util
import json
import os
from pathlib import Path
import re
import stat
import sqlite3
import sys
import tempfile

_spec = importlib.util.spec_from_file_location('controller_packet', Path(__file__).with_name('macos-controller-account-packet.py'))
packet = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(packet)
JOBS = Path('/Library/Application Support/Shantytown/jobs')
NAME = 'eph-task-000001'
OLD = 'e985f7f8850b3c3cc6aaaba71865d426e9472a1737ac8c1a8be945e99e990890'
PLAN = '21d6283211c709dd3370bf558b5c2e69529a59849b63d48d5b1af1b6c7b5c10a'
IDENTITY = {'version', 'principal', 'uid', 'gid', 'controller_uid', 'username', 'home', 'root'}
LAUNCH = {'controller_root', 'workspace', 'settings', 'harness', 'request', 'argv', 'environment', 'review'}
ENVIRONMENT = {'PATH', 'LANG', 'LC_ALL', 'TERM', 'CODEX_HOME', 'CLAUDE_CONFIG_DIR'}


def encoded(value):
    return (json.dumps(value, indent=2, sort_keys=True) + '\n').encode()


def pinned(path, pin):
    raw = packet.regular(path, 1048576)
    if not re.fullmatch('[0-9a-f]{64}', pin or '') or packet.digest(raw) != pin:
        raise ValueError('explicit input digest mismatch')
    return raw


def document(raw):
    def unique(pairs):
        result = {}
        for key, value in pairs:
            if key in result:
                raise ValueError('duplicate JSON key')
            result[key] = value
        return result
    return json.loads(raw, object_pairs_hook=unique)


def candidate(old_raw, launch_raw):
    old, extra = document(old_raw), document(launch_raw)
    if packet.digest(old_raw) != OLD or not isinstance(old, dict) or set(old) != IDENTITY:
        raise ValueError('exact original provision-only manifest required')
    if not isinstance(extra, dict) or set(extra) != LAUNCH:
        raise ValueError('add exactly the eight launch fields; no identity edits')
    row = dict(old, **extra)
    if row['harness'] not in ('codex', 'claude') or row['review'] is not None:
        raise ValueError('initial synthetic task requires a supported harness and no review policy')
    req = row['request']
    if (not isinstance(req, dict) or set(req) != {'job', 'item', 'repo', 'pr', 'checkout', 'head', 'request_id'}
            or req['job'] != 'task'):
        raise ValueError('exact synthetic task request required')
    home = Path(old['home'])
    for key in ('controller_root', 'workspace', 'settings'):
        value = row[key]
        if (not isinstance(value, str) or not Path(value).is_absolute()
                or '..' in Path(value).parts or any(c in value for c in '\r\n\0')):
            raise ValueError('invalid launch path')
    root, controller = Path(old['root']), Path(row['controller_root'])
    if root.is_relative_to(controller) or controller.is_relative_to(root):
        raise ValueError('controller and job stores must be separate')
    for key in ('workspace', 'settings'):
        if not Path(row[key]).is_relative_to(home) or Path(row[key]) == home:
            raise ValueError('workspace/settings must be inside the private job home')
    argv = row['argv']
    if (not isinstance(argv, list) or not 1 <= len(argv) <= 128
            or any(not isinstance(x, str) or not x or len(x) > 8192 or '\0' in x for x in argv)
            or not Path(argv[0]).is_absolute()):
        raise ValueError('invalid pinned argv')
    env = row['environment']
    if (not isinstance(env, dict) or set(env) - ENVIRONMENT
            or any(not isinstance(v, str) or '\0' in v for v in env.values())):
        raise ValueError('environment is not allowlisted')
    for key in ('CODEX_HOME', 'CLAUDE_CONFIG_DIR'):
        if key in env and (not Path(env[key]).is_relative_to(home) or '..' in Path(env[key]).parts):
            raise ValueError('harness config must belong to the job')
    # Keep the original eight fields' serialized bytes, not merely their values.
    prefix = old_raw[:old_raw.rfind(b'}')].rstrip()
    addition = encoded(extra).strip()[1:-1].strip()
    return prefix + b',\n  ' + addition + b'\n}\n'


def append_event(raw, event):
    """Preserve earlier serialized fields/events, adding bytes before JSON closes."""
    value = document(raw)
    addition = b'\n'.join(b'    ' + line for line in encoded(event).rstrip().splitlines())
    if 'migrations' not in value:
        return raw[:raw.rfind(b'}')].rstrip() + b',\n  "migrations": [\n' + addition + b'\n  ]\n}\n'
    # This script introduces migrations as the final field; refuse foreign edits.
    end = raw.rfind(b']')
    if end < 0 or raw[end:].strip() != b']\n}':
        raise ValueError('unknown migration receipt serialization')
    return raw[:end].rstrip() + b',\n' + addition + b'\n  ]\n}\n'


def atomic(path, previous, raw, mode):
    packet.protected(path)
    if packet.regular(path, 1048576) != previous:
        raise ValueError('destination changed; preserve it')
    fd, temporary = tempfile.mkstemp(prefix='.' + path.name + '.', dir=path.parent)
    try:
        with os.fdopen(fd, 'wb') as stream:
            os.fchmod(stream.fileno(), mode)
            stream.write(raw)
            stream.flush()
            os.fsync(stream.fileno())
        os.replace(temporary, path)
        fd = os.open(path.parent, os.O_RDONLY)
        try:
            os.fsync(fd)
        finally:
            os.close(fd)
    finally:
        if os.path.exists(temporary):
            os.unlink(temporary)


def no_launch():
    used = JOBS / 'used'
    if not used.exists() and not used.is_symlink():
        return  # Protected catalog makes absence a positive observation under the lock.
    packet.protected(used)
    if not used.is_dir():
        raise ValueError('unknown consumed receipt inventory')
    paths = list(used.iterdir())
    if len(paths) > 1025:
        raise ValueError('unbounded consumed receipt inventory')
    for path in paths:
        packet.protected(path)
        if path.name == '.lock':
            if not stat.S_ISREG(path.lstat().st_mode):
                raise ValueError('invalid launch lock')
            continue
        if path.suffix != '.json':
            raise ValueError('unknown launch receipt entry')
        old = document(packet.regular(path, 65536))
        if not isinstance(old, dict) or type(old.get('uid')) is not int:
            raise ValueError('unknown consumed receipt')
        if old['uid'] == 51001 or old.get('principal') == NAME:
            raise ValueError('slot already launched; forward-only, never reuse')


def before_activation():
    for path in (Path('/private/var/lib/shantytown/job-stores') / NAME,
                 Path('/private/etc/sudoers.d/st-job-slots')):
        if path.exists() or path.is_symlink():
            raise ValueError('metadata/policy already applied; explicit forward recovery required')


def native(row, launch=False):
    users, groups = packet.inventories()
    packet.unique(row, users, groups)
    expected = {'UniqueID': '51001', 'PrimaryGroupID': '51001', 'NFSHomeDirectory': row['home'],
                'UserShell': '/usr/bin/false', 'IsHidden': '1', 'AuthenticationAuthority': ';DisabledUser;'}
    if packet.properties('/Users/_stjobs', list(expected)) != expected:
        raise ValueError('original disabled native account differs')
    result = packet.run(['/usr/sbin/dseditgroup', '-o', 'checkmember', '-m', '_stjobs', 'admin'], ok=False)
    if result.returncode == 0 or result.stdout.strip() != 'no _stjobs is NOT a member of admin':
        raise ValueError('job non-admin membership not proved')
    packet.protected(Path('/private' + row['home']).parent)
    for key in ('home', 'workspace', 'settings') if launch else ('home',):
        path = Path('/private' + row[key]) if row[key].startswith('/var/') else Path(row[key])
        info = path.lstat()
        kind = stat.S_ISREG if key == 'settings' else stat.S_ISDIR
        if not kind(info.st_mode) or info.st_uid != 51001 or info.st_gid != 51001 or stat.S_IMODE(info.st_mode) != (0o600 if key == 'settings' else 0o700):
            raise ValueError('private job state ownership/mode differs')
    if launch:
        packet.protected(Path(row['argv'][0]))


def receipt():
    path = JOBS / 'account-receipts/51001.json'
    packet.protected(path)
    if stat.S_IMODE(path.lstat().st_mode) != 0o600:
        raise ValueError('account receipt must remain root0600')
    raw = packet.regular(path, 1048576)
    data = document(raw)
    expected = {'principal': NAME, 'uid': 51001, 'sha256': OLD, 'state': 'complete'}
    if (not isinstance(data, dict) or set(data) - set(expected) - {'migrations'}
            or any(data.get(k) != v or type(data.get(k)) != type(v) for k, v in expected.items())
            or not isinstance(data.get('migrations', []), list)):
        raise ValueError('original complete nonreuse receipt not proved')
    return path, raw, data


def binding(launch_raw, ledger, ledger_pin, request_id, item_raw, max_attempts):
    """Check operator-pinned, read-only evidence; never contact a live store."""
    row = document(launch_raw)
    req = row['request']
    if (not re.fullmatch('[0-9a-f]{64}', request_id or '') or not isinstance(req.get('item'), str)
            or not re.fullmatch('[A-Za-z0-9][A-Za-z0-9_.-]{0,127}', req['item'])
            or not isinstance(max_attempts, int) or max_attempts < 2):
        raise ValueError('explicit synthetic request, created item and remaining retry budget required')
    evidence = document(item_raw)
    if isinstance(evidence, list) and len(evidence) == 1:
        evidence = evidence[0]
    if not isinstance(evidence, dict) or evidence.get('id') != req['item']:
        raise ValueError('Seeds item readback does not match the launch item')
    if any(Path(str(ledger) + suffix).exists() for suffix in ('-wal', '-shm', '-journal')):
        raise ValueError('use a consistent standalone ledger snapshot, not a live database')
    raw = pinned(ledger, ledger_pin)
    connection = sqlite3.connect(ledger.resolve().as_uri() + '?mode=ro&immutable=1', uri=True)
    try:
        connection.execute('PRAGMA query_only=ON')
        results = connection.execute(
            'SELECT request_id,kind,repo,state,work_item,attempts,claimed_until FROM requests WHERE request_id=? LIMIT 2',
            (request_id,)).fetchall()
    finally:
        connection.close()
    if pinned(ledger, ledger_pin) != raw:
        raise ValueError('ledger snapshot changed during verification')
    if len(results) != 1:
        raise ValueError('synthetic request is not unique in the ledger')
    rid, kind, repo, state, item, attempts, claimed = results[0]
    if (kind != 'install' or repo != req['repo'] or state != 'refused' or item != req['item']
            or type(attempts) is not int or not 1 <= attempts < max_attempts or claimed is not None):
        raise ValueError('ledger does not prove refused phase A, same item/repo and an available retry')
    return {'request_id': rid, 'item': item, 'attempts': attempts,
            'ledger_sha256': ledger_pin, 'item_readback_sha256': packet.digest(item_raw)}


def perform(mode, old_raw, plan_raw, new_raw, commit, operator, evidence=None):
    packet.protected(JOBS)
    before_activation()
    no_launch()
    manifest, plan = JOBS / (NAME + '.json'), JOBS / 'slots.json'
    for path in (manifest, plan):
        packet.protected(path)
    rp, rr, rd = receipt()
    history = rd.get('migrations', [])
    new_pin = packet.digest(new_raw)
    new_plan = document(plan_raw)
    new_plan['slots'][NAME]['sha256'] = new_pin
    new_plan_raw = encoded(new_plan)
    expected_manifest, expected_plan = (old_raw, plan_raw) if mode == 'expand' else (new_raw, new_plan_raw)
    if packet.regular(manifest) != expected_manifest or packet.regular(plan) != expected_plan:
        raise ValueError('current catalog differs from exact expected pins; retain it')
    if mode == 'expand' and history:
        raise ValueError('migration already attempted; no automatic retry/reuse')
    if mode == 'rollback' and (len(history) != 2 or history[0].get('action') != 'expand-prepared'
            or history[1].get('action') != 'expand-complete'
            or any(e.get('old_sha256') != OLD or e.get('new_sha256') != new_pin for e in history)):
        raise ValueError('complete matching expansion journal required')
    native(document(new_raw), launch=mode == 'expand')
    archive = JOBS / 'migrations'
    packet.directory(archive)
    packet.directory(archive / 'archive')
    packet.install(archive / 'archive' / (NAME + '.e985f7f8.json'), old_raw, 0o444)
    packet.install(archive / 'archive/slots.21d62832.json', plan_raw, 0o444)
    # JSON fields are immutable; append events, retaining every earlier event.
    event = {'action': mode + '-prepared', 'old_sha256': OLD, 'new_sha256': new_pin,
             'reviewed_commit': commit, 'operator': operator,
             'time': datetime.now(timezone.utc).isoformat(), 'binding': evidence}
    prepared = append_event(rr, event)
    atomic(rp, rr, prepared, 0o600)
    target_manifest, target_plan = (new_raw, new_plan_raw) if mode == 'expand' else (old_raw, plan_raw)
    # Two atomic files cannot form one filesystem transaction. The prepared
    # receipt is durable first; any interruption stays UNKNOWN and refuses retry.
    atomic(manifest, expected_manifest, target_manifest, 0o644)
    atomic(plan, expected_plan, target_plan, 0o644)
    complete = dict(event, action=mode + '-complete', time=datetime.now(timezone.utc).isoformat())
    atomic(rp, prepared, append_event(prepared, complete), 0o600)
    print(mode.upper() + ' COMPLETE ' + new_pin + '; readiness=false; original identity/receipt retained')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('mode', choices=('preview', 'expand', 'rollback'))
    parser.add_argument('--original', type=Path, required=True)
    parser.add_argument('--inventory', type=Path, required=True)
    parser.add_argument('--launch-fields', type=Path, required=True)
    parser.add_argument('--launch-fields-sha256', required=True)
    parser.add_argument('--reviewed-commit', required=True)
    parser.add_argument('--operator', required=True)
    parser.add_argument('--ledger-snapshot', type=Path, required=True)
    parser.add_argument('--ledger-sha256', required=True)
    parser.add_argument('--synthetic-request-id', required=True)
    parser.add_argument('--item-readback', type=Path, required=True)
    parser.add_argument('--item-readback-sha256', required=True)
    parser.add_argument('--max-attempts', type=int, required=True)
    args = parser.parse_args()
    try:
        if not re.fullmatch('[0-9a-f]{40}', args.reviewed_commit) or not re.fullmatch('[A-Za-z0-9_.-]{1,64}', args.operator):
            raise ValueError('explicit reviewed commit and operator required')
        old, plan = pinned(args.original, OLD), pinned(args.inventory, PLAN)
        launch = pinned(args.launch_fields, args.launch_fields_sha256)
        new = candidate(old, launch)
        evidence = binding(launch, args.ledger_snapshot, args.ledger_sha256, args.synthetic_request_id,
                           pinned(args.item_readback, args.item_readback_sha256), args.max_attempts)
        if args.mode == 'preview':
            print('CANDIDATE ' + packet.digest(new) + '; no host inspection; readiness=UNPROVED')
            return 0
        if sys.platform != 'darwin' or os.geteuid() != 0:
            raise ValueError('operator must run this with sudo on macOS')
        os.umask(0o077)
        packet.protected(JOBS)
        # Share the launcher's lock; a running launch can never race migration.
        packet.directory(JOBS / 'used')
        lock = JOBS / 'used/.lock'
        fd = os.open(lock, os.O_RDWR | os.O_CREAT | os.O_NOFOLLOW, 0o600)
        try:
            packet.protected(lock)
            fcntl.flock(fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
            perform(args.mode, old, plan, new, args.reviewed_commit, args.operator, evidence)
        finally:
            os.close(fd)
        return 0
    except (OSError, ValueError, TypeError, KeyError, sqlite3.Error) as error:
        print('REFUSED: ' + str(error), file=sys.stderr)
        return 1


if __name__ == '__main__':
    raise SystemExit(main())
