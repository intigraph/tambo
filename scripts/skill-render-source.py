#!/usr/bin/env python3
"""Compile an explicit skill tree to native render data; no graph or install I/O."""
import argparse
import base64
import hashlib
import json
import os
from pathlib import Path
import stat

MAX_BYTES = 8 * 1024 * 1024
MAX_FILES = 512
MAX_ENTRIES = 1024


def compile_skill(root):
    """Read through no-follow directory capabilities, refusing partial trees."""
    files = {}
    budget = MAX_BYTES
    entries = 0

    def walk(directory, prefix):
        nonlocal budget, entries
        before = os.fstat(directory)
        names = []
        with os.scandir(directory) as children:
            for entry in children:
                entries += 1
                if entries > MAX_ENTRIES:
                    raise ValueError('skill source exceeds entry budget')
                names.append(entry.name)
        for name in sorted(names):
            if (not name.isascii() or name.startswith('.') or
                    any(c in name for c in '\\:{}\x00') or
                    any(ord(c) < 32 or ord(c) == 127 for c in name) or name.endswith((' ', '.'))):
                raise ValueError('unsafe skill source path')
            path = prefix + name
            if len(path) > 1024:
                raise ValueError('skill source path exceeds budget')
            info = os.stat(name, dir_fd=directory, follow_symlinks=False)
            if stat.S_ISDIR(info.st_mode):
                child = os.open(name, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW,
                                dir_fd=directory)
                try:
                    if (os.fstat(child).st_dev, os.fstat(child).st_ino) != (info.st_dev, info.st_ino):
                        raise ValueError('skill directory changed')
                    walk(child, path + '/')
                finally:
                    os.close(child)
                continue
            if not stat.S_ISREG(info.st_mode):
                raise ValueError('skill source must contain regular files only')
            if len(files) >= MAX_FILES or info.st_size > budget:
                raise ValueError('skill source exceeds budget')
            mode = stat.S_IMODE(info.st_mode)
            if mode not in (0o644, 0o755):
                raise ValueError('skill source mode must be 0644 or 0755')
            fd = os.open(name, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK,
                         dir_fd=directory)
            with os.fdopen(fd, 'rb') as stream:
                opened = os.fstat(stream.fileno())
                if (opened.st_dev, opened.st_ino, opened.st_mode, opened.st_size,
                        opened.st_mtime_ns, opened.st_ctime_ns) != (
                            info.st_dev, info.st_ino, info.st_mode, info.st_size,
                            info.st_mtime_ns, info.st_ctime_ns):
                    raise ValueError('skill source changed before read')
                content = stream.read(budget + 1)
                after = os.fstat(stream.fileno())
            if len(content) > budget or (opened.st_size, opened.st_mode,
                    opened.st_mtime_ns, opened.st_ctime_ns) != (
                        after.st_size, after.st_mode, after.st_mtime_ns, after.st_ctime_ns):
                raise ValueError('skill source changed during read or exceeds budget')
            budget -= len(content)
            files[path] = (content, mode)
        after = os.fstat(directory)
        if (before.st_mtime_ns, before.st_ctime_ns) != (after.st_mtime_ns, after.st_ctime_ns):
            raise ValueError('skill directory changed during read')

    directory = os.open(root, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
    try:
        walk(directory, '')
    finally:
        os.close(directory)
    if 'SKILL.md' not in files:
        raise ValueError('SKILL.md required')
    content, mode = files.pop('SKILL.md')
    if mode != 0o644 or len(content) > 256 * 1024:
        raise ValueError('SKILL.md must be 0644 and at most 256 KiB')
    text = content.decode('utf-8')
    # The skill is query DATA, never template source. Template-like text stays raw.
    literal = json.dumps(text, ensure_ascii=False)
    return {
        'query': 'SELECT ?content WHERE { VALUES ?content { ' + literal + ' } }',
        'format': 'markdown', 'layout': 'single', 'path': 'SKILL.md',
        'template': '{{ rows[0].content }}', 'provenance': False,
        'includes': [dict(path=path, sha256='sha256:' + hashlib.sha256(data).hexdigest(),
                          base64=base64.b64encode(data).decode(), mode=mode)
                     for path, (data, mode) in sorted(files.items())],
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('source', type=Path)
    args = parser.parse_args()
    try:
        result = compile_skill(args.source)
    except (ValueError, OSError, UnicodeError) as error:
        parser.exit(1, f'skill source refused: {error}\n')
    print(json.dumps(result, ensure_ascii=False, sort_keys=True))


if __name__ == '__main__':
    main()
