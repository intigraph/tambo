#!/usr/bin/env python3
"""Disposable archive controls; never run foreign code or install a runtime."""
import hashlib
import importlib.util
import io
import json
from pathlib import Path
import struct
import tarfile
import tempfile
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location('runtime_builder', Path(__file__).with_name('build-macos-job-runtime.py'))
p = importlib.util.module_from_spec(spec)
spec.loader.exec_module(p)
stage_spec = importlib.util.spec_from_file_location('runtime_stage', Path(__file__).with_name('macos-job-runtime-stage.py'))
stage = importlib.util.module_from_spec(stage_spec)
stage_spec.loader.exec_module(stage)
MACHO = struct.pack('<II', 0xfeedfacf, 0x0100000c) + b'synthetic native header; never executed'
LAUNCHER = b'#!/opt/shantytown-job-launcher/bin/python3 -BIS\n# synthetic fixture\n'


def tar(path, files, links=None):
    with tarfile.open(path, 'w:gz') as value:
        for name, raw in files.items():
            member = tarfile.TarInfo(name)
            member.size = len(raw)
            value.addfile(member, io.BytesIO(raw))
        for name, target in (links or {}).items():
            member = tarfile.TarInfo(name)
            member.type = tarfile.SYMTYPE
            member.linkname = target
            value.addfile(member)


def writable(root):
    for path in root.rglob('*'):
        if path.is_dir():
            path.chmod(0o755)
    root.chmod(0o755)


class RuntimeTests(unittest.TestCase):
    def setUp(self):
        self.private = tempfile.TemporaryDirectory()
        self.root = Path(self.private.name)
        self.addCleanup(self.private.cleanup)
        self.addCleanup(writable, self.root)
        self.python, self.app, self.sd = [self.root / (name + '.tar.gz') for name in ('python', 'app', 'sd')]
        self.python_files = {'python/bin/python3.12': MACHO, 'python/lib/python3.12/os.py': b'#os',
                            'python/lib/python3.12/encodings/__init__.py': b'#encodings',
                            'python/lib/python3.12/site-packages/evil.pth': b'import evil',
                            'python/lib/python3.12/__pycache__/os.pyc': b'bytecode'}
        self.links = {'python/bin/python3': 'python3.12'}
        self.app_files = {'shantytown-abc/shantytown/' + name: b'# public fixture' for name in
                          ['__init__.py', 'job_launch.py', 'job_metadata.py']}
        self.app_files.update({'shantytown-abc/scripts/st-job-launcher': LAUNCHER,
                               'shantytown-abc/LICENSE': b'license'})
        tar(self.python, self.python_files, self.links)
        tar(self.app, self.app_files)
        tar(self.sd, {'sd-seeds-ai-v1-aarch64-apple-darwin/sd': MACHO,
                      'sd-seeds-ai-v1-aarch64-apple-darwin/LICENSE': b'license'})
        self.pins = self.root / 'sources.json'
        self.write_pins()

    def write_pins(self):
        self.pins.write_text(json.dumps({'python': {'sha256': p.digest(self.python)},
            'shantytown': {'sha256': p.digest(self.app), 'commit': 'abc'},
            'seeds': {'sha256': p.digest(self.sd), 'version': '1'}}))

    def build(self, output):
        return p.build(self.python, self.app, self.sd, output, self.pins)

    def test_deterministic_complete_regular_inventory(self):
        for parent in ('first', 'second'):
            (self.root / parent).mkdir()
        one, two = [self.root / parent / 'runtime' for parent in ('first', 'second')]
        self.assertEqual(self.build(one), self.build(two))
        manifest = json.loads((one / 'manifest.json').read_text())
        self.assertEqual(manifest['platform'], 'macos')
        self.assertFalse(any(x.is_symlink() for x in one.rglob('*')))
        self.assertEqual((one / 'runtime/bin/python3').read_bytes(), MACHO)
        self.assertFalse(any('site-packages' in x or '__pycache__' in x for x in manifest['files']))
        self.assertTrue(all(not x.stat().st_mode & 0o222 for x in one.rglob('*')))
        for folder in (one, two):
            p.pack(folder, folder.with_name('runtime.tar.gz'))
        self.assertEqual(p.digest(one.with_name('runtime.tar.gz')), p.digest(two.with_name('runtime.tar.gz')))
        # Stage accepts only the fixed release root name. Build the same fixture
        # under that name and exercise its pinned read without privileged writes.
        artifact = self.root / 'macos-arm64-job-runtime'
        pin = self.build(artifact)
        archive = artifact.with_name(artifact.name + '.tar.gz')
        p.pack(artifact, archive)
        files = stage.read_archive(archive, p.digest(archive), pin)
        self.assertIn('runtime/bin/python3', files)
        with self.assertRaisesRegex(ValueError, 'manifest pin differs'):
            stage.read_archive(archive, p.digest(archive), '0' * 64)
        with self.assertRaisesRegex(ValueError, 'archive digest mismatch'):
            stage.read_archive(archive, '0' * 64, pin)

    def test_tampered_input_refuses_before_output(self):
        self.python.write_bytes(b'not the reviewed archive')
        output = self.root / 'output'
        with self.assertRaisesRegex(ValueError, 'digest mismatch'):
            self.build(output)
        self.assertFalse(output.exists())

    def test_wrong_architecture_refuses(self):
        self.python_files['python/bin/python3.12'] = b'#!/bin/sh\nforeign wrapper'
        tar(self.python, self.python_files, self.links)
        self.write_pins()
        with self.assertRaisesRegex(ValueError, 'native arm64'):
            self.build(self.root / 'output')

    def test_archive_traversal_and_link_escape_refuse(self):
        for name, target in [('python/lib/../../escape', b'foreign'), ('python/lib/evil', None)]:
            with self.subTest(name=name):
                if target:
                    tar(self.python, dict(self.python_files, **{name: target}), self.links)
                else:
                    tar(self.python, self.python_files, dict(self.links, **{name: '../../../outside'}))
                self.write_pins()
                with self.assertRaises(ValueError):
                    self.build(self.root / 'output')
                self.assertFalse((self.root / 'output').exists())

    def test_occupied_output_is_preserved(self):
        output = self.root / 'output'
        output.mkdir()
        marker = output / 'keep'
        marker.write_text('retained')
        with self.assertRaises(FileExistsError):
            self.build(output)
        self.assertEqual(marker.read_text(), 'retained')

    def test_stager_refuses_wrong_platform_before_writes(self):
        from types import SimpleNamespace
        with patch.object(stage.sys, 'platform', 'linux'), patch.object(stage, 'directory') as write:
            with self.assertRaisesRegex(ValueError, 'native macOS'):
                stage.stage(SimpleNamespace())
            write.assert_not_called()

    def test_root_build_refuses_before_input_reads(self):
        with patch.object(p.os, 'geteuid', return_value=0):
            with self.assertRaisesRegex(ValueError, 'unprivileged'):
                self.build(self.root / 'output')


if __name__ == '__main__':
    unittest.main()
