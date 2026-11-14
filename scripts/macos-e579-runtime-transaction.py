#!/usr/bin/env python3
"""Finite operator transaction. Source/release approval is NOT native admission.

A0 inspects only. A1/A2/A3 require a separately admitted root-owned packet.
No account, key, policy, launchd, catalog or readiness mutation is implemented.
"""
import argparse
import contextlib
import datetime as dt
import fcntl
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import platform
import re
import shutil
import stat
import subprocess
import sys

HERE = Path(__file__).resolve().parent
OLD_TOOL = '35c646c9090cfbf29679ac8bc2754754f49454b1213a98ed1fd37d5c0446436a'
NEW_TOOL = 'af3730602aad22542e775c71614476792b2cfc8e6626ef7e7561904ee0fc32b3'
OLD_RUNTIME = '33de07c81561a45b3fd49a8f88160b9761300732cc99c16b21670d5c026acb9a'
NEW_RUNTIME = '25cadd5a13a3f8021f9fb11bedfb789e815b5cb10141962f4fdba9ca35fb85f1'
NEW_ARCHIVE = '42780e8f25740f79bce6d21ae13d855ba058bae54e1a652ae978cef1939dd040'
HELPER = '03f4ab60513c17677aaef7243efef69396f9210b12be0abd5468566e001aa516'
CATALOG = '21d6283211c709dd3370bf558b5c2e69529a59849b63d48d5b1af1b6c7b5c10a'
JOB = 'e985f7f8850b3c3cc6aaaba71865d426e9472a1737ac8c1a8be945e99e990890'
CONTROLLER = 'aff7d04c3b67f6cb068c6ee92c3f02012ed724a7d55b6138fc68036c6874dd9c'


def sha(raw):
    return hashlib.sha256(raw).hexdigest()


def load_module(name):
    spec = importlib.util.spec_from_file_location(name, HERE / (name + '.py'))
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def admission(raw, pin, phase, now):
    if sha(raw) != pin:
        raise ValueError('admission bytes differ')
    def unique(pairs):
        result = {}
        for key, value in pairs:
            if key in result:
                raise ValueError('duplicate admission key')
            result[key] = value
        return result
    a = json.loads(raw, object_pairs_hook=unique)
    fields = {'version', 'transaction', 'phase', 'expires_at', 'freeze', 'sources', 'tool', 'archive', 'a0_sha256'}
    sources = {'macos-e579-runtime-transaction.py', 'macos-controller-account-packet.py', 'macos-job-runtime-stage.py'}
    if (not isinstance(a, dict) or set(a) != fields or a['version'] != 1
            or a['transaction'] != 'e579-0141' or a['phase'] != phase
            or phase not in ('a1', 'a2', 'a3') or a['freeze'] != 'all-job-admission-disabled'
            or not isinstance(a['sources'], dict) or set(a['sources']) != sources
            or not isinstance(a['a0_sha256'], str) or not re.fullmatch('[0-9a-f]{64}', a['a0_sha256'])):
        raise ValueError('exact phase/freeze/source/A0 admission required')
    for value in a['sources'].values():
        if not isinstance(value, str) or not re.fullmatch('[0-9a-f]{64}', value):
            raise ValueError('exact reviewed source hashes required')
    for name in ('tool', 'archive'):
        if not isinstance(a[name], str) or not Path(a[name]).is_absolute():
            raise ValueError('exact absolute admitted input paths required')
    expiry = dt.datetime.strptime(a['expires_at'], '%Y-%m-%dT%H:%M:%SZ').replace(tzinfo=dt.timezone.utc)
    if not now < expiry <= now + dt.timedelta(hours=2):
        raise ValueError('admission expired or unbounded')
    return a


class Transaction:
    def __init__(self, base, opt, helper, policy, uid=0):
        # Alternate paths exist ONLY for imported fixture tests; CLI has no root override.
        self.base, self.opt, self.helper, self.policy, self.uid = base, opt, helper, policy, uid
        self.tool = base / 'bin/tambo-controller-e579'
        self.runtime = opt / 'shantytown-job-launcher'
        self.state = base / 'e579-replacement-0141'
        self.old_tree = opt / '.e579-0141-old-runtime'
        self.backup = opt / '.e579-0141-runtime-backup'
        self.proof = opt / '.e579-0141-runtime-restore-proof'
        self.pending = opt / '.e579-0141-new-runtime'
        self.failed_tree = opt / '.e579-0141-retained-new-runtime'
        self.native = lambda: None

    def safe(self, path, mode=None):
        for p in (path, *path.parents):
            m = p.lstat()
            if m.st_uid != self.uid or m.st_mode & 0o022 or not (
                    stat.S_ISREG(m.st_mode) or stat.S_ISDIR(m.st_mode)):
                raise ValueError('unsafe protected path: ' + str(p))
        m = path.lstat()
        if stat.S_ISREG(m.st_mode) and m.st_nlink != 1:
            raise ValueError('hard-linked protected file')
        if mode is not None and stat.S_IMODE(m.st_mode) != mode:
            raise ValueError('protected mode differs')

    def read(self, path, mode=None, limit=268435456):
        self.safe(path, mode)
        fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW)
        with os.fdopen(fd, 'rb') as f:
            m = os.fstat(f.fileno())
            if not stat.S_ISREG(m.st_mode) or m.st_size > limit:
                raise ValueError('bounded regular file required')
            raw = f.read(limit + 1)
            if len(raw) > limit:
                raise ValueError('file grew beyond bound')
            return raw

    def absent(self, path):
        if path.exists() or path.is_symlink():
            raise ValueError('occupied path must be retained: ' + str(path))
        self.safe(path.parent)

    def sync(self, path):
        fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW)
        try:
            os.fsync(fd)
        finally:
            os.close(fd)

    def create(self, path, raw, mode):
        self.absent(path)
        fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, mode)
        with os.fdopen(fd, 'wb') as f:
            os.fchmod(f.fileno(), mode)
            f.write(raw)
            f.flush()
            os.fsync(f.fileno())
        self.sync(path.parent)

    def tree(self, root, pin):
        self.safe(root, 0o755)
        raw = self.read(root / '.artifact.json')
        if sha(raw) != pin:
            raise ValueError('runtime predecessor manifest differs')
        manifest = json.loads(raw)
        if set(manifest) != {'version', 'platform', 'files'} or manifest['version'] != 1 or manifest['platform'] != 'macos':
            raise ValueError('runtime manifest schema differs')
        wanted = {n[8:]: h for n, h in manifest['files'].items() if n.startswith('runtime/')}
        if not wanted or any(n.startswith('/') or any(x in ('', '.', '..') for x in n.split('/')) for n in wanted):
            raise ValueError('unsafe runtime manifest paths')
        wanted['.artifact.json'] = pin
        dirs = {str(p) for n in wanted for p in Path(n).parents if str(p) != '.'}
        found, found_dirs = {}, set()
        for p in root.rglob('*'):
            self.safe(p)
            name = str(p.relative_to(root))
            if p.is_dir():
                self.safe(p, 0o755)
                found_dirs.add(name)
            else:
                self.safe(p, 0o755 if name.startswith('bin/') else 0o644)
                found[name] = sha(self.read(p))
        if found != wanted or found_dirs != dirs:
            raise ValueError('runtime inventory/hash/mode differs')
        return sha(json.dumps(found, sort_keys=True).encode())

    def prerequisites(self):
        jobs = self.base / 'jobs'
        fixed = {jobs / 'slots.json': CATALOG, jobs / 'eph-task-000001.json': JOB, self.helper: HELPER}
        for path, pin in fixed.items():
            if sha(self.read(path)) != pin:
                raise ValueError('fixed original packet/helper changed')
        controller = json.loads(self.read(self.base / 'controllers/51000.json', 0o600, 4096))
        job = json.loads(self.read(jobs / 'account-receipts/51001.json', 0o600, 4096))
        if controller != dict(version=1, uid=51000, packet_sha256=CONTROLLER, state='complete'):
            raise ValueError('complete controller reservation not proved')
        if job != dict(principal='eph-task-000001', uid=51001, sha256=JOB, state='complete'):
            raise ValueError('complete job reservation not proved')
        for p in (jobs / 'identity.json', jobs / 'used', jobs / 'migrations', self.policy,
                  self.base / 'controllers/controller-packet-rolled-back',
                  self.opt / '.shantytown-job-launcher.pending', self.helper.parent / '.st-job-launcher.pending'):
            self.absent(p)
        # Only exact known state is admitted. No private key contents are read.
        self.native()
        return sha(json.dumps({str(p): sha(self.read(p)) for p in fixed}, sort_keys=True).encode())

    @contextlib.contextmanager
    def locks(self):
        # Same locks as account setup, account provisioner, policy and runtime installer.
        paths = [self.base / '.controller-packet-lock', self.base / 'jobs/.account-lock',
                 self.policy.parent / '.st-job-slots.lock', self.opt / '.shantytown-job-runtime.lock']
        with contextlib.ExitStack() as stack:
            for path in paths:
                self.safe(path.parent)
                fd = os.open(path, os.O_RDWR | os.O_CREAT | os.O_NOFOLLOW, 0o600)
                f = stack.enter_context(os.fdopen(fd, 'r+b'))
                self.safe(path, 0o600)
                fcntl.flock(f.fileno(), fcntl.LOCK_EX | fcntl.LOCK_NB)
            yield

    def observe(self):
        stamp = self.prerequisites()
        if sha(self.read(self.tool, 0o755)) != OLD_TOOL:
            raise ValueError('old controller tool differs')
        self.tree(self.runtime, OLD_RUNTIME)
        for p in (self.state, self.old_tree, self.backup, self.proof, self.pending, self.failed_tree):
            self.absent(p)
        return dict(version=1, transaction='e579-0141', prerequisites=stamp,
                    tool=OLD_TOOL, runtime=OLD_RUNTIME, native_ready=False)

    def journal(self, phase, stamp):
        raw = json.dumps(dict(version=1, phase=phase, prerequisites=stamp), sort_keys=True).encode() + b'\n'
        p = self.state / ('receipt-' + phase + '.json')
        self.create(p, raw, 0o600)  # Each phase is exclusive and retained, never overwritten.

    def attest(self, stamp, expected):
        receipt = dict(version=1, transaction='e579-0141', prerequisites=stamp,
                       tool=OLD_TOOL, runtime=OLD_RUNTIME, native_ready=False)
        if expected is not None and sha((json.dumps(receipt, sort_keys=True) + '\n').encode()) != expected:
            raise ValueError('admitted A0 receipt differs')

    def a1(self, tool, expected=None):
        with self.locks():
            before = self.observe()
            self.attest(before['prerequisites'], expected)
            raw = self.read(tool, 0o755)
            if sha(raw) != NEW_TOOL:
                raise ValueError('new public tool hash differs')
            self.state.mkdir(mode=0o700)
            self.safe(self.state, 0o700)
            self.journal('prepared', before['prerequisites'])
            self.create(self.state / 'old-tool', self.read(self.tool, 0o755), 0o755)
            # Protected backup AND independently copied restore proof before replacement.
            for target in (self.backup, self.proof):
                shutil.copytree(self.runtime, target)
                self.tree(target, OLD_RUNTIME)
                for path in sorted(target.rglob('*'), reverse=True):
                    self.sync(path)
                self.sync(target)
            staged = self.tool.parent / '.tambo-controller-e579-0141.pending'
            self.create(staged, raw, 0o755)
            if self.prerequisites() != before['prerequisites'] or sha(self.read(self.tool)) != OLD_TOOL:
                raise ValueError('tool compare-and-swap predecessor changed')
            os.replace(staged, self.tool)
            self.sync(self.tool.parent)
            if sha(self.read(self.tool, 0o755)) != NEW_TOOL:
                raise ValueError('new tool readback differs; retain evidence')
            self.journal('tool', before['prerequisites'])

    def stamp(self):
        self.safe(self.state, 0o700)
        row = json.loads(self.read(self.state / 'receipt-prepared.json', 0o600, 4096))
        if set(row) != {'version', 'phase', 'prerequisites'} or row['version'] != 1 or row['phase'] != 'prepared':
            raise ValueError('transaction receipt schema differs')
        if self.prerequisites() != row['prerequisites']:
            raise ValueError('prerequisites changed after transaction began')
        self.tree(self.backup, OLD_RUNTIME)
        self.tree(self.proof, OLD_RUNTIME)
        if sha(self.read(self.state / 'old-tool', 0o755)) != OLD_TOOL:
            raise ValueError('protected old tool backup differs')
        return row['prerequisites']

    def a2(self, archive, stage, expected=None):
        with self.locks():
            stamp = self.stamp()
            self.attest(stamp, expected)
            if json.loads(self.read(self.state / 'receipt-tool.json', 0o600, 4096)) != dict(version=1, phase='tool', prerequisites=stamp):
                raise ValueError('tool phase receipt differs')
            if sha(self.read(self.tool, 0o755)) != NEW_TOOL:
                raise ValueError('installed tool CAS changed')
            self.tree(self.runtime, OLD_RUNTIME)
            for path in (self.pending, self.old_tree, self.failed_tree):
                self.absent(path)
            self.read(archive)  # Require protected input parents before parsing.
            files = stage.read_archive(archive, NEW_ARCHIVE, NEW_RUNTIME)
            if sha(files['launcher']) != HELPER:
                raise ValueError('fixed helper must not change')
            self.pending.mkdir(mode=0o755)
            for name, raw in sorted(files.items()):
                if not name.startswith('runtime/'):
                    continue
                path = self.pending / name[8:]
                path.parent.mkdir(mode=0o755, parents=True, exist_ok=True)
                self.create(path, raw, 0o755 if name.startswith('runtime/bin/') else 0o644)
            self.create(self.pending / '.artifact.json', files['manifest.json'], 0o644)
            self.tree(self.pending, NEW_RUNTIME)
            for path in sorted(self.pending.rglob('*'), reverse=True):
                self.sync(path)
            self.sync(self.pending)
            if self.prerequisites() != stamp:
                raise ValueError('runtime CAS prerequisites changed')
            self.tree(self.runtime, OLD_RUNTIME)
            self.journal('runtime-prepared', stamp)
            os.rename(self.runtime, self.old_tree)
            self.sync(self.opt)
            self.journal('old-moved', stamp)
            os.rename(self.pending, self.runtime)
            self.sync(self.opt)
            self.tree(self.runtime, NEW_RUNTIME)
            self.journal('complete', stamp)

    def a3(self, expected=None):
        # Never resumes A1/A2. Only reverses exact recognizable states before use.
        with self.locks():
            stamp = self.stamp()
            self.attest(stamp, expected)
            self.absent(self.state / 'receipt-rolled-back.json')
            tool = sha(self.read(self.tool, 0o755))
            if tool not in (OLD_TOOL, NEW_TOOL):
                raise ValueError('unrecognized tool state; no inverse')
            if self.old_tree.exists() or self.old_tree.is_symlink():
                self.tree(self.old_tree, OLD_RUNTIME)
                if self.runtime.exists() or self.runtime.is_symlink():
                    self.tree(self.runtime, NEW_RUNTIME)
                    self.absent(self.failed_tree)
                    os.rename(self.runtime, self.failed_tree)
                os.rename(self.old_tree, self.runtime)
                self.sync(self.opt)
            self.tree(self.runtime, OLD_RUNTIME)
            if tool == NEW_TOOL:
                self.create(self.state / 'retained-new-tool', self.read(self.tool), 0o755)
                pending = self.tool.parent / '.tambo-controller-e579-0141.inverse'
                self.create(pending, self.read(self.state / 'old-tool'), 0o755)
                if sha(self.read(self.tool)) != NEW_TOOL or self.prerequisites() != stamp:
                    raise ValueError('inverse tool CAS changed')
                os.replace(pending, self.tool)
                self.sync(self.tool.parent)
            if sha(self.read(self.tool, 0o755)) != OLD_TOOL:
                raise ValueError('inverse tool readback differs')
            self.journal('rolled-back', stamp)


def native_observer(tx):
    controller = load_module('macos-controller-account-packet')
    for row in (
            dict(uid=51000, gid=51000, username='_stctl', home='/var/lib/shantytown/controllers/_stctl'),
            dict(uid=51001, gid=51001, username='_stjobs', home='/var/lib/shantytown/agents/crew-eph-task-000001')):
        controller.native_verify(row)
    users, groups = controller.inventories()
    if any(60000 <= uid <= 60063 for uid in list(users.values()) + list(groups.values())):
        raise ValueError('expansion identities present; transaction scope ended')
    for path in (Path('/Library/LaunchDaemons/org.shantytown.controller.plist'),
                 Path('/private/var/lib/shantytown/controllers/_stctl/root')):
        if path.exists() or path.is_symlink():
            raise ValueError('controller activation/store state exists')
    # Require a successful whole launchd inventory, not an ambiguous missing-label error.
    inventory = controller.run(['/bin/launchctl', 'print', 'system']).stdout
    if 'system = {' not in inventory or 'org.shantytown.controller' in inventory:
        raise ValueError('controller launchd absence not proved')
    for principal in ('_stctl', '_stjobs'):
        result = controller.run(['/usr/bin/sudo', '-n', '-l', '-U', principal], ok=False)
        if result.returncode != 1 or not re.fullmatch(
                r'User ' + principal + r' is not allowed to run sudo on [^\n]+\.\n?',
                result.stdout + result.stderr):
            raise ValueError('complete isolated-principal sudo denial not proved')
    processes = controller.run(['/bin/ps', '-axo', 'uid=,pid=,command=']).stdout
    if not processes.strip():
        raise ValueError('empty root process instrument')
    for line in processes.splitlines():
        parts = line.split(None, 2)
        if len(parts) != 3 or not parts[0].isdigit():
            raise ValueError('process inventory malformed')
        uid = int(parts[0])
        if uid in (51000, 51001) or 60000 <= uid <= 60063 or str(tx.runtime) in parts[2]:
            raise ValueError('active isolated/runtime process')
    opened = controller.run(['/usr/sbin/lsof', '-nP', '-Fpun'])
    if opened.stderr or not opened.stdout.startswith('p'):
        raise ValueError('root open-file coverage unknown')
    for line in opened.stdout.splitlines():
        if line.startswith('u'):
            if not line[1:].isdigit():
                raise ValueError('open-file UID instrument malformed')
            if int(line[1:]) in (51000, 51001) or 60000 <= int(line[1:]) <= 60063:
                raise ValueError('isolated principal has open files')
        if line.startswith('n') and any(str(p) in line[1:] for p in (tx.runtime, tx.tool, tx.helper)):
            raise ValueError('protected executable/runtime is in use')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('phase', choices=['a0', 'a1', 'a2', 'a3'])
    parser.add_argument('--admission', type=Path)
    parser.add_argument('--admission-sha256')
    parser.add_argument('--tool', type=Path)
    parser.add_argument('--archive', type=Path)
    args = parser.parse_args()
    if sys.platform != 'darwin' or platform.machine() != 'arm64' or os.geteuid() != 0:
        raise ValueError('separately admitted root operator on native arm64 macOS required')
    tx = Transaction(Path('/Library/Application Support/Shantytown'), Path('/opt'),
                     Path('/usr/local/libexec/st-job-launcher'), Path('/private/etc/sudoers.d/st-job-slots'))
    for name in ('macos-e579-runtime-transaction.py', 'macos-controller-account-packet.py', 'macos-job-runtime-stage.py'):
        tx.safe(HERE / name)
    tx.native = lambda: native_observer(tx)
    if args.phase == 'a0':
        print(json.dumps(tx.observe(), sort_keys=True))
        return
    if args.admission is None:
        raise ValueError('native mutation admission is separate and required')
    raw = tx.read(args.admission, 0o600, 16384)
    a = admission(raw, args.admission_sha256, args.phase, dt.datetime.now(dt.timezone.utc))
    for name in ('macos-e579-runtime-transaction.py', 'macos-controller-account-packet.py', 'macos-job-runtime-stage.py'):
        if sha(tx.read(HERE / name)) != a['sources'][name]:
            raise ValueError('reviewed protected source changed')
    if args.phase == 'a1':
        if args.tool is None or str(args.tool) != a['tool']:
            raise ValueError('exact admitted tool path required')
        tx.a1(args.tool, a['a0_sha256'])
    elif args.phase == 'a2':
        if args.archive is None or str(args.archive) != a['archive']:
            raise ValueError('exact admitted runtime archive path required')
        tx.a2(args.archive, load_module('macos-job-runtime-stage'), a['a0_sha256'])
    else:
        tx.a3(a['a0_sha256'])
    print(json.dumps(dict(phase=args.phase, native_ready=False, activation='HELD')))


if __name__ == '__main__':
    try:
        main()
    except (OSError, ValueError, KeyError, TypeError, subprocess.SubprocessError) as exc:
        print('REFUSED: ' + str(exc) + '; retain every partial artifact; no automatic replay', file=sys.stderr)
        raise SystemExit(1)
