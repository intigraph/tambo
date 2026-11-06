#!/usr/bin/env python3
"""One explicitly invoked receiver delivery, at most twice per immutable job config.

Source packet only: never scheduled or invoked by the renderer. A native operator
must admit the complete private config/source/event before running this command.
The counter is separate from, and never edits, the Chaski receiver ledger.
"""
import argparse
import fcntl
import hashlib
import json
import os
from pathlib import Path
import stat
import subprocess
import sys

FIELDS = {'version', 'max_attempts', 'receiver_script', 'receiver_sha256',
          'routing_config', 'routing_sha256', 'event', 'event_sha256', 'state'}


def digest(raw):
    return hashlib.sha256(raw).hexdigest()


def pinned(path, expected):
    path = Path(path)
    info = path.lstat()
    if not path.is_absolute() or not stat.S_ISREG(info.st_mode) or info.st_nlink != 1:
        raise ValueError('pinned input must be an absolute regular non-linked file')
    raw = path.read_bytes()
    if digest(raw) != expected:
        raise ValueError('pinned input changed')
    return raw


def load_job(path):
    raw = path.read_bytes()
    job = json.loads(raw)
    if (not isinstance(job, dict) or set(job) != FIELDS or type(job['version']) is not int or job['version'] != 1
            or type(job['max_attempts']) is not int or job['max_attempts'] != 2):
        raise ValueError('job-local max_attempts must be exactly 2')
    pinned(job['receiver_script'], job['receiver_sha256'])
    pinned(job['routing_config'], job['routing_sha256'])
    event = pinned(job['event'], job['event_sha256'])
    if not isinstance(json.loads(event), dict):
        raise ValueError('one complete emitted Chaski event required')
    return job, digest(raw), event


def private_file(path, flags):
    fd = os.open(path, flags | os.O_NOFOLLOW, 0o600)
    info = os.fstat(fd)
    if (not stat.S_ISREG(info.st_mode) or info.st_nlink != 1 or info.st_uid != os.getuid()
            or stat.S_IMODE(info.st_mode) != 0o600):
        os.close(fd)
        raise ValueError('private own-UID single-link state required')
    return fd


def deliver(job_path):
    job, pin, event = load_job(job_path)
    state = Path(job['state'])
    if not state.is_absolute() or '..' in state.parts:
        raise ValueError('fixed absolute job state path required')
    # mkdir is create-only. Existing exact state is retained, never reset/adopted.
    try:
        state.mkdir(mode=0o700)
    except FileExistsError:
        pass
    info = state.lstat()
    if (not stat.S_ISDIR(info.st_mode) or info.st_uid != os.getuid()
            or stat.S_IMODE(info.st_mode) != 0o700):
        raise ValueError('private own-UID state directory required')
    lock = private_file(state / 'lock', os.O_RDWR | os.O_CREAT)
    try:
        fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        record = state / 'attempts.json'
        if record.exists() or record.is_symlink():
            fd = private_file(record, os.O_RDONLY)
            with os.fdopen(fd) as stream:
                receipt = json.load(stream)
            if (set(receipt) != {'job_sha256', 'attempts'} or receipt['job_sha256'] != pin
                    or type(receipt['attempts']) is not int or not 1 <= receipt['attempts'] <= 2):
                raise ValueError('retained job reservation changed or malformed')
            count = receipt['attempts']
        else:
            # A retained lock with no receipt could be a failed first reservation.
            # New lock contents distinguish a fresh state from an interrupted one.
            if os.fstat(lock).st_size:
                raise ValueError('interrupted reservation; preserve for operator recovery')
            count = 0
        if count >= job['max_attempts']:
            raise ValueError('job retry budget exhausted; no receiver invocation')
        # Mark before writing the receipt: interruption can refuse, never grant a retry.
        os.write(lock, b'reserved\n'); os.fsync(lock)
        pending = state / 'attempts.pending'
        fd = private_file(pending, os.O_WRONLY | os.O_CREAT | os.O_EXCL)
        with os.fdopen(fd, 'w') as stream:
            json.dump({'job_sha256': pin, 'attempts': count + 1}, stream)
            stream.flush(); os.fsync(stream.fileno())
        os.replace(pending, record)
        directory = os.open(state, os.O_RDONLY | os.O_DIRECTORY)
        try: os.fsync(directory)
        finally: os.close(directory)
        print('reserved attempt=' + str(count + 1) + ' max_attempts=2', flush=True)
        # No shell or endpoint shortcut. The receiver owns validation/ledger state.
        result = subprocess.run([sys.executable, '-B', job['receiver_script'], '--config',
                                 job['routing_config'], 'deliver'], input=event, timeout=900,
                                check=False)
        return result.returncode
    finally:
        os.close(lock)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--job-config', type=Path, required=True)
    args = parser.parse_args()
    try:
        return deliver(args.job_config)
    except (ValueError, OSError, subprocess.TimeoutExpired) as exc:
        parser.exit(1, 'REFUSED: ' + str(exc) + '\n')


if __name__ == '__main__':
    raise SystemExit(main())
