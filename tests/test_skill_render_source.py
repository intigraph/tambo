#!/usr/bin/env python3
"""Offline producer controls; native rendering is a separate paired control."""
import base64
import importlib.util
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location('producer', Path(__file__).resolve().parents[1] / 'scripts/skill-render-source.py')
producer = importlib.util.module_from_spec(spec)
spec.loader.exec_module(producer)


class SourceTests(unittest.TestCase):
    def fixture(self, root):
        (root / 'SKILL.md').write_text('---\nname: example\n---\n{{ untouched }}\nUnicode λ\n')
        (root / 'SKILL.md').chmod(0o644)

    def test_raw_and_binary_deterministic(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            self.fixture(root)
            (root / 'tool').write_bytes(b'\x00\xff\x01')
            (root / 'tool').chmod(0o755)
            result = producer.compile_skill(root)
            self.assertEqual(result, producer.compile_skill(root))
            self.assertEqual(base64.b64decode(result['includes'][0]['base64']), b'\x00\xff\x01')
            self.assertEqual(result['includes'][0]['mode'], 0o755)
            self.assertEqual(result['template'], '{{ rows[0].content }}')
            self.assertFalse(result['provenance'])

    def test_refuses_symlink_even_if_target_is_inside(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            self.fixture(root)
            (root / 'alias').symlink_to('SKILL.md')
            with self.assertRaises(ValueError):
                producer.compile_skill(root)

    def test_missing_entry_and_unsafe_path(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            with self.assertRaises(ValueError):
                producer.compile_skill(root)
            self.fixture(root)
            (root / '{name}').write_text('unsafe')
            with self.assertRaises(ValueError):
                producer.compile_skill(root)

    def test_mode_and_size_refuse(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            self.fixture(root)
            (root / 'SKILL.md').chmod(0o600)
            with self.assertRaises(ValueError):
                producer.compile_skill(root)
            (root / 'SKILL.md').chmod(0o644)
            (root / 'huge').write_bytes(b'x' * (producer.MAX_BYTES + 1))
            with self.assertRaises(ValueError):
                producer.compile_skill(root)


    def test_special_file_root_symlink_and_invalid_utf8(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            source = root / 'source'
            source.mkdir()
            self.fixture(source)
            (root / 'alias').symlink_to(source, target_is_directory=True)
            with self.assertRaises(OSError):
                producer.compile_skill(root / 'alias')
            os.mkfifo(source / 'pipe')
            with self.assertRaises(ValueError):
                producer.compile_skill(source)
            (source / 'pipe').unlink()
            (source / 'SKILL.md').write_bytes(b'\xff')
            with self.assertRaises(UnicodeError):
                producer.compile_skill(source)



    def test_replacement_between_stat_and_open_refuses(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            self.fixture(root)
            native_open = os.open

            def change_before_open(path, flags, **kwargs):
                if path == 'SKILL.md':
                    (root / 'SKILL.md').write_text('changed after metadata check')
                return native_open(path, flags, **kwargs)

            with patch.object(producer.os, 'open', side_effect=change_before_open):
                with self.assertRaisesRegex(ValueError, 'changed before read'):
                    producer.compile_skill(root)



    def test_empty_directories_count_toward_streamed_entry_budget(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            self.fixture(root)
            for name in ['one', 'two']:
                (root / name).mkdir()
            with patch.object(producer, 'MAX_ENTRIES', 3):
                self.assertEqual(producer.compile_skill(root)['path'], 'SKILL.md')
            with patch.object(producer, 'MAX_ENTRIES', 2):
                with self.assertRaisesRegex(ValueError, 'entry budget'):
                    producer.compile_skill(root)



if __name__ == '__main__':
    unittest.main()
