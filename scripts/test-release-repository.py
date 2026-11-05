#!/usr/bin/env python3
"""Replay the actual canonical-repository release step without publishing.

# arming: just check and CI; cargo metadata is an owned fixture, not a build.
"""
import json
import os
from pathlib import Path
import subprocess
import tempfile
import tomllib
import unittest

ROOT = Path(__file__).resolve().parents[1]


def release_step():
    workflow = (ROOT / '.github/workflows/release.yml').read_text()
    start = workflow.index('      - name: Require the canonical repository and matching tag\n')
    block = workflow[start:].split('        run: |\n', 1)[1]
    lines = []
    for row in block.splitlines():
        if row and not row.startswith('          '):
            break
        lines.append(row[10:])
    return '\n'.join(lines)


class CanonicalRelease(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory(prefix='release-repository-')
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name)
        self.env_file = self.root / 'env'
        version = tomllib.loads((ROOT / 'Cargo.toml').read_text())['package']['version']
        self.version = version
        cargo = self.root / 'cargo'
        cargo.write_text('#!/bin/sh\nprintf \'%s\\n\' \'' + json.dumps(
            {'packages': [{'name': 'tambo', 'version': version}]}) + "'\n")
        cargo.chmod(0o755)

    def run_step(self, repository, tag=None):
        env = dict(os.environ, PATH=str(self.root) + ':/usr/bin:/bin',
                   VERSION=tag or 'v' + self.version, GITHUB_ENV=str(self.env_file))
        if repository is None:
            env.pop('GITHUB_REPOSITORY', None)
        else:
            env['GITHUB_REPOSITORY'] = repository
        return subprocess.run(['bash', '-c', release_step()], env=env,
                              capture_output=True, text=True, timeout=10)

    def test_canonical_repository_and_matching_tag_pass(self):
        result = self.run_step('intigraph/tambo')
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(self.env_file.read_text(), 'TAMBO_PUBLISH_VERSION=' + self.version + '\n')

    def test_foreign_legacy_near_miss_and_missing_repository_refuse(self):
        for repository in ('foreign/tambo', 'scbrown/tambo', 'intigraph/tambo-fork', None):
            with self.subTest(repository=repository):
                self.assertNotEqual(self.run_step(repository).returncode, 0)
                self.assertFalse(self.env_file.exists())

    def test_wrong_tag_still_refuses(self):
        self.assertNotEqual(self.run_step('intigraph/tambo', 'v0.0.0-invalid').returncode, 0)
        self.assertFalse(self.env_file.exists())


if __name__ == '__main__':
    unittest.main()
