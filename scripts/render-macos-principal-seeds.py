#!/usr/bin/env python3
"""Render private e579 candidates only; never execute, enroll, install or arm."""
import argparse
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import plistlib
import re
from urllib.parse import urlsplit

SD = '/opt/shantytown-job-launcher/bin/sd'
ACCOUNTS = {
    '_stctl': (51000, '/var/lib/shantytown/controllers/_stctl'),
    '_stjobs': (51001, '/var/lib/shantytown/agents/crew-eph-task-000001'),
}
BASE = {'username', 'uid', 'gid', 'home', 'principal', 'introducer'}
CONFIG = {'endpoint', 'graph', 'ca_file', 'ca_sha256', 'signing_key_file',
          'signing_session', 'signing_introducer', 'seeds_config_dir'}


def absolute(value):
    if (not isinstance(value, str) or not value.startswith('/') or value == '/'
            or any(ord(c) < 32 or ord(c) == 127 for c in value)
            or str(PurePosixPath(value)) != value or '..' in PurePosixPath(value).parts):
        raise ValueError('canonical absolute path required')
    return value


def name(value):
    if not isinstance(value, str) or not re.fullmatch(r'[A-Za-z0-9][A-Za-z0-9_.:-]{0,127}', value):
        raise ValueError('explicit identifier required')
    return value


def url(value):
    if not isinstance(value, str) or any(ord(c) < 33 for c in value):
        raise ValueError('credential-free HTTPS endpoint required')
    p = urlsplit(value)
    if p.scheme != 'https' or not p.hostname or p.username or p.password or p.query or p.fragment:
        raise ValueError('credential-free HTTPS endpoint required')
    return value


def render(data, phase):
    expected = BASE if phase == 'key-init' else BASE | CONFIG
    if not isinstance(data, dict) or set(data) != expected:
        raise ValueError('exact phase input fields required; no secrets or extra fields')
    user = data['username']
    if not isinstance(user, str) or user not in ACCOUNTS:
        raise ValueError('only the reviewed controller and first reserved slot are supported')
    uid, home = ACCOUNTS[user]
    if (type(data['uid']) is not int or type(data['gid']) is not int
            or data['uid'] != uid or data['gid'] != uid or data['home'] != home):
        raise ValueError('account UID/GID/home differs from reviewed packet')
    principal, introducer = name(data['principal']), name(data['introducer'])
    if (introducer == principal or (user == '_stjobs' and principal != 'eph-task-000001')
            or (user == '_stctl' and principal == 'eph-task-000001')):
        raise ValueError('distinct controller/job principal and external introducer required')
    label = 'org.shantytown.e579.key-init.' + user.lstrip('_')
    if phase == 'key-init':
        plist = {'Label': label, 'Disabled': True, 'RunAtLoad': False, 'KeepAlive': False,
                 'UserName': user, 'GroupName': user, 'WorkingDirectory': home,
                 'EnvironmentVariables': {'HOME': home, 'XDG_CONFIG_HOME': home + '/.config',
                                          'PATH': '/usr/bin:/bin'},
                 'ProgramArguments': [SD, 'key', 'init', '--session', principal,
                                      '--agent', principal, '--introducer', introducer],
                 'Umask': 0o077,
                 'StandardOutPath': '/Library/Logs/Shantytown/e579/' + user + '.public.log',
                 'StandardErrorPath': '/Library/Logs/Shantytown/e579/' + user + '.error.log'}
        return {label + '.plist': plistlib.dumps(plist, sort_keys=True)}
    url(data['endpoint'])
    graph = data['graph']
    if not isinstance(graph, str) or not re.fullmatch(r'https://[^\s<>"\\]+', graph):
        raise ValueError('explicit HTTPS project graph IRI required')
    for field in ('ca_file', 'signing_key_file', 'seeds_config_dir'):
        absolute(data[field])
    if not re.fullmatch(r'[0-9a-f]{64}', data['ca_sha256']):
        raise ValueError('public CA SHA256 required')
    if not any(data['signing_key_file'].startswith(h + '/.config/seeds/keys/')
               for h in (home, '/private' + home)):
        raise ValueError('signing key must stay inside this UID own home keys directory')
    if data['signing_session'] != principal or data['signing_introducer'] != introducer:
        raise ValueError('enrollment session/introducer must match this principal')
    config_dir = data['seeds_config_dir']
    protected = '/Library/Application Support/Shantytown/seeds-config/' + user
    if config_dir != protected:
        raise ValueError('distinct reviewed root-owned configuration directory required')
    # JSON basic strings are valid TOML basic strings for these validated inputs.
    quote = lambda value: json.dumps(value, ensure_ascii=True)
    values = {'url': data['endpoint'], **{k: data[k] for k in (
        'ca_file', 'ca_sha256', 'signing_key_file', 'signing_session', 'signing_introducer')}}
    config = '[quipu]\n' + ''.join(k + ' = ' + quote(v) + '\n' for k, v in values.items())
    config += '\n[project]\ngraph = ' + quote(graph) + '\n'
    receipt = {'source_only': True, 'uid': uid, 'principal': principal,
               'seeds_config_dir': config_dir, 'identity_binding': {'seeds_config_dir': config_dir},
               'native_required': {'directory_owner': 'root:wheel', 'directory_mode': '0755',
                                   'config_owner': 'root:wheel', 'config_mode': '0644',
                                   'key_owner_uid': uid, 'key_mode': '0600'},
               'readiness': False, 'installed': False}
    return {'seeds/config.toml': config.encode(),
            'installation-requirements.json': (json.dumps(receipt, indent=2) + '\n').encode()}


def load(path):
    def unique(pairs):
        result = {}
        for k, v in pairs:
            if k in result:
                raise ValueError('duplicate input field')
            result[k] = v
        return result
    return json.loads(path.read_text(), object_pairs_hook=unique)


def write_candidates(output, files):
    # Caller-owned private staging only. No host installation or chmod/chown of existing paths.
    output.mkdir(mode=0o700)
    for relative, raw in files.items():
        target = output / relative
        if target.parent != output:
            target.parent.mkdir(mode=0o700)
        fd = os.open(target, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600)
        with os.fdopen(fd, 'wb') as stream:
            stream.write(raw)
            stream.flush()
            os.fsync(stream.fileno())
        print(relative + ' sha256=' + hashlib.sha256(raw).hexdigest())
    print('SOURCE CANDIDATES ONLY; no key generation, installation or launch; readiness FALSE')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('phase', choices=('key-init', 'config'))
    parser.add_argument('--input', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    try:
        files = render(load(args.input), args.phase)
        write_candidates(args.output, files)
    except (ValueError, OSError, TypeError) as exc:
        parser.exit(1, 'REFUSED: ' + str(exc) + '\n')


if __name__ == '__main__':
    main()
