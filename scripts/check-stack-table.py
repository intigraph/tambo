#!/usr/bin/env python3
"""Check exact marked stack tables locally or across the published stack."""

import argparse
import hashlib
from pathlib import Path
import re
import sys
from urllib.error import URLError
from urllib.request import Request, urlopen

ROOT = Path(__file__).resolve().parents[1]
REPOS = ("tambo", "quipu", "camayoc", "quechua", "chaski", "bobbin", "yupana", "seeds", "shantytown", "shuttle", "rikuy", "desire-path")
START = b"<!-- stack-table:start -->"
END = b"<!-- stack-table:end -->"
MAX_BYTES = 256 * 1024
BLOCK = re.compile(
    rb"^<!-- stack-table:start -->\n(.*?)^<!-- stack-table:end -->(?:\n|$)",
    re.MULTILINE | re.DOTALL,
)


def marked_table(data: bytes, label: str) -> bytes:
    """Require one complete block; preserve every byte between its markers."""
    if data.count(START) != 1 or data.count(END) != 1:
        raise ValueError(f"{label}: expected exactly one pair of stack-table markers")
    match = BLOCK.search(data)
    if match is None:
        raise ValueError(f"{label}: malformed or reversed stack-table markers")
    body = match.group(1)
    # The guarded overview may lead with prose. Compare that prose verbatim
    # along with the table; never normalize it away when checking copies.
    table = body.strip(b"\n").rsplit(b"\n\n", 1)[-1].splitlines()
    if (len(table) < 3 or not table[0].startswith(b"| tool |")
            or table[1] != b"|---|---|"
            or any(not row.startswith(b"|") or not row.endswith(b"|") for row in table)
            or not body.endswith(b"\n")):
        raise ValueError(f"{label}: marked block is not a complete stack overview")
    return body


def remote_readme(repo: str) -> bytes:
    """Fetch only the fixed public repository set, with time and size bounds."""
    if repo not in REPOS:
        raise ValueError("repository is outside the stack table audit")
    url = f"https://raw.githubusercontent.com/scbrown/{repo}/main/README.md"
    request = Request(url, headers={"User-Agent": "caboodle-stack-table-check"})
    with urlopen(request, timeout=10) as response:
        data = response.read(MAX_BYTES + 1)
    if len(data) > MAX_BYTES:
        raise ValueError(f"{repo}: README exceeds the audit size limit")
    return data


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("readmes", nargs="*", type=Path, help="local README copies")
    parser.add_argument("--canonical", type=Path, default=ROOT / "docs/stack-table.md")
    parser.add_argument("--remote", action="store_true", help="also audit all twelve public main branches")
    args = parser.parse_args()
    try:
        canonical = marked_table(args.canonical.read_bytes(), str(args.canonical))
    except (OSError, ValueError) as error:
        print(f"FAIL: {error}", file=sys.stderr)
        return 1
    digest = hashlib.sha256(canonical).hexdigest()
    failures = 0
    targets = [(str(path), lambda path=path: path.read_bytes())
               for path in (args.readmes or [ROOT / "README.md"])]
    if args.remote:
        targets.extend((repo, lambda repo=repo: remote_readme(repo)) for repo in REPOS)
    for label, read in targets:
        try:
            actual = marked_table(read(), label)
            if actual != canonical:
                raise ValueError(f"{label}: stack table differs from docs/stack-table.md")
        except (OSError, URLError, ValueError) as error:
            print(f"FAIL: {error}", file=sys.stderr)
            failures += 1
        else:
            print(f"PASS: {label} sha256={digest}")
    print(f"checked={len(targets)} failed={failures}")
    return 1 if failures else 0


if __name__ == "__main__":
    raise SystemExit(main())
