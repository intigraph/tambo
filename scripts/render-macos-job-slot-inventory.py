#!/usr/bin/env python3
"""Render a reviewable provision-only job-slot inventory. Writes files only.

No account, sudo, host path or network is touched. The output is a DRAFT input
for `tambo job-slots --directory <output>` (review mode). Identity fields only:
these manifests carry no launch capability.
"""
import argparse
import hashlib
import json
from pathlib import Path

KINDS = ('review', 'merge', 'triage', 'task')


def manifest(principal, uid, controller_uid, username):
    # Field order and serialization match the reviewed initial manifest bytes.
    return (json.dumps({
        'version': 1, 'principal': principal, 'uid': uid, 'gid': uid,
        'controller_uid': controller_uid, 'username': username,
        'home': '/var/lib/shantytown/agents/crew-' + principal,
        'root': '/var/lib/shantytown/job-stores/' + principal,
    }, indent=2) + '\n').encode()


def render(first_uid, count, controller_uid, kind, retained, output):
    if (kind not in KINDS or not 10000 <= first_uid or not 1 <= count <= 1024
            or controller_uid < 10000 or first_uid <= controller_uid < first_uid + count):
        raise ValueError('explicit bounded plan with UIDs >= 10000 required')
    files, slots, uids = {}, {}, []
    for path in retained:
        raw = path.read_bytes()  # carried byte-for-byte; never re-serialized
        row = json.loads(raw)
        files[path.name] = raw
        slots[row['principal']] = {'uid': row['uid'], 'sha256': hashlib.sha256(raw).hexdigest()}
        uids.append(row['uid'])
    for uid in range(first_uid, first_uid + count):
        suffix = format(uid, '06x')  # slot name is its UID in hex: checkable by eye
        principal = 'eph-' + kind + '-' + suffix
        raw = manifest(principal, uid, controller_uid, '_stjobs_' + suffix)
        if principal in slots or uid in uids:
            raise ValueError('principal or UID repeats')
        files[principal + '.json'] = raw
        slots[principal] = {'uid': uid, 'sha256': hashlib.sha256(raw).hexdigest()}
        uids.append(uid)
    plan = {'version': 1, 'uid_min': min(uids), 'uid_max': max(uids), 'slots': slots}
    files['slots.json'] = (json.dumps(plan, indent=2) + '\n').encode()
    output.mkdir(parents=True)  # exclusive: never overwrite a reviewed inventory
    for name, raw in files.items():
        with (output / name).open('xb') as stream:
            stream.write(raw)
    return hashlib.sha256(files['slots.json']).hexdigest(), len(slots)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--first-uid', type=int, required=True)
    parser.add_argument('--count', type=int, required=True)
    parser.add_argument('--controller-uid', type=int, required=True)
    parser.add_argument('--kind', choices=KINDS, required=True)
    parser.add_argument('--retain', type=Path, action='append', default=[],
                        help='existing manifest carried unchanged (repeatable)')
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    pin, total = render(args.first_uid, args.count, args.controller_uid, args.kind,
                        args.retain, args.output)
    print(json.dumps({'slots': total, 'slots_json_sha256': pin}))
