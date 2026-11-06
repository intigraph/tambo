#!/usr/bin/env python3
"""Offline candidate controls: never execute sd, launchctl, sudo or key generation."""
import importlib.util
import json
import os
from pathlib import Path
import plistlib
import subprocess
import sys
import tempfile
try:
    import tomllib
except ModuleNotFoundError:  # Native account scripts retain the Python3.9 surface.
    import configparser
    class tomllib:
        @staticmethod
        def loads(raw):
            parser = configparser.ConfigParser(interpolation=None)
            parser.read_string(raw)
            return {section: {k: json.loads(v) for k, v in parser[section].items()}
                    for section in parser.sections()}
import unittest

ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / 'scripts/render-macos-principal-seeds.py'
spec = importlib.util.spec_from_file_location('principal_seeds', SCRIPT)
p = importlib.util.module_from_spec(spec)
spec.loader.exec_module(p)


def inputs(user='_stctl', config=False):
    uid, home = p.ACCOUNTS[user]
    principal = 'controller-fixture' if user == '_stctl' else 'eph-task-000001'
    row = dict(username=user, uid=uid, gid=uid, home=home, principal=principal,
               introducer='operator-fixture')
    if config:
        row.update(endpoint='https://graph.example.invalid', graph='https://example.invalid/project/test',
                   ca_file='/Library/Application Support/Shantytown/ca/public.pem', ca_sha256='a' * 64,
                   signing_key_file=home + '/.config/seeds/keys/' + principal + '.key',
                   signing_session=principal, signing_introducer=row['introducer'],
                   seeds_config_dir='/Library/Application Support/Shantytown/seeds-config/' + user)
    return row


class CandidateTests(unittest.TestCase):
    def test_each_launch_job_binds_exact_uid_account_and_own_home(self):
        for user in p.ACCOUNTS:
            row = inputs(user)
            files = p.render(row, 'key-init')
            self.assertEqual(len(files), 1)
            job = plistlib.loads(next(iter(files.values())))
            self.assertEqual(job['UserName'], user)
            self.assertEqual(job['GroupName'], user)
            self.assertEqual(job['WorkingDirectory'], row['home'])
            self.assertEqual(job['EnvironmentVariables']['XDG_CONFIG_HOME'], row['home'] + '/.config')
            self.assertTrue(job['Disabled'])
            self.assertFalse(job['RunAtLoad'])
            self.assertFalse(job['KeepAlive'])
            self.assertEqual(job['Umask'], 0o077)
            self.assertEqual(job['ProgramArguments'], [p.SD, 'key', 'init', '--session', row['principal'],
                                                     '--agent', row['principal'], '--introducer', row['introducer']])
            self.assertNotIn('seeds_config_dir', str(job))

    def test_config_preserves_ca_and_own_enrollment_context(self):
        for user in p.ACCOUNTS:
            row = inputs(user, True)
            files = p.render(row, 'config')
            cfg = tomllib.loads(files['seeds/config.toml'].decode())
            for field in ('ca_file', 'ca_sha256', 'signing_key_file', 'signing_session', 'signing_introducer'):
                self.assertEqual(cfg['quipu'][field], row[field])
            self.assertEqual(cfg['quipu']['url'], row['endpoint'])
            self.assertEqual(cfg['project']['graph'], row['graph'])
            receipt = json.loads(files['installation-requirements.json'])
            self.assertEqual(receipt['identity_binding']['seeds_config_dir'], row['seeds_config_dir'])
            self.assertFalse(receipt['installed'])
            self.assertFalse(receipt['readiness'])

    def test_native_private_var_alias_retains_own_key(self):
        row = inputs(config=True)
        row['signing_key_file'] = '/private' + row['signing_key_file']
        cfg = tomllib.loads(p.render(row, 'config')['seeds/config.toml'].decode())
        self.assertEqual(cfg['quipu']['signing_key_file'], row['signing_key_file'])

    def test_identity_cross_uid_and_extra_secret_fields_refuse(self):
        for key, value in [('uid', 501), ('uid', True), ('gid', 51001), ('username', 'root'),
                           ('home', '/tmp/home'), ('principal', 'eph-task-000001'),
                           ('introducer', 'controller-fixture'), ('principal', 'bad\nvalue'),
                           ('private_key', 'fixture-secret')]:
            with self.subTest(key=key):
                row = inputs(); row[key] = value
                with self.assertRaises(ValueError): p.render(row, 'key-init')

    def test_foreign_key_unprotected_redirect_and_invalid_trust_refuse(self):
        for key, value in [('signing_key_file', inputs('_stjobs', True)['signing_key_file']),
                           ('signing_key_file', inputs('_stctl')['home'] + '/.config/seeds/keys/../foreign'),
                           ('signing_session', 'other'), ('signing_introducer', 'other'),
                           ('seeds_config_dir', '/tmp/config'), ('ca_sha256', 'bad'),
                           ('ca_file', '/tmp/../ca'), ('endpoint', 'http://example.invalid'),
                           ('endpoint', 'https://user:password@example.invalid'),
                           ('graph', 'https://example.invalid/\nother')]:
            with self.subTest(key=key, value=value):
                row = inputs(config=True); row[key] = value
                with self.assertRaises(ValueError): p.render(row, 'config')

    def test_cli_creates_only_private_candidates_and_refuses_existing_output(self):
        with tempfile.TemporaryDirectory() as d:
            directory = Path(d); source = directory / 'input.json'; output = directory / 'output'
            source.write_text(json.dumps(inputs(config=True)))
            env = dict(os.environ, PATH='/nonexistent', PYTHONDONTWRITEBYTECODE='1')
            cmd = [sys.executable, '-B', str(SCRIPT), 'config', '--input', str(source), '--output', str(output)]
            result = subprocess.run(cmd, env=env, capture_output=True, text=True)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertIn('SOURCE CANDIDATES ONLY', result.stdout)
            self.assertEqual(output.stat().st_mode & 0o777, 0o700)
            config = output / 'seeds/config.toml'; before = config.read_bytes()
            self.assertEqual(config.stat().st_mode & 0o777, 0o600)
            self.assertNotEqual(subprocess.run(cmd, env=env, capture_output=True).returncode, 0)
            self.assertEqual(config.read_bytes(), before)
            link = directory / 'link'; link.symlink_to(output, target_is_directory=True)
            cmd[-1] = str(link)
            self.assertNotEqual(subprocess.run(cmd, env=env, capture_output=True).returncode, 0)
            self.assertEqual(config.read_bytes(), before)

    def test_duplicate_fields_and_invalid_input_create_nothing(self):
        with tempfile.TemporaryDirectory() as d:
            directory = Path(d); source = directory / 'input.json'
            source.write_text('{"uid":51000,"uid":501}')
            with self.assertRaises(ValueError): p.load(source)
            source.write_text(json.dumps({**inputs(), 'uid': 501}))
            output = directory / 'absent'
            result = subprocess.run([sys.executable, '-B', str(SCRIPT), 'key-init', '--input', str(source),
                                     '--output', str(output)], capture_output=True)
            self.assertNotEqual(result.returncode, 0)
            self.assertFalse(output.exists())


if __name__ == '__main__': unittest.main()
