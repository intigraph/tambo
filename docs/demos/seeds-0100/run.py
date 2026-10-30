#!/usr/bin/env python3
"""Display effective Seeds pins from two source manifests; never installs."""
import sys
import tomllib
from pathlib import Path

print("Source configuration demonstration; no installed service is changed.")
for label, path in zip(("BASE", "CANDIDATE"), sys.argv[1:], strict=True):
    m = tomllib.loads(Path(path).read_text())
    print("\n" + label)
    for field in ("repo", "version", "tag", "asset"):
        print(f"{field}: {m[field]}")
    for target, digest in sorted(m["sha256"].items()):
        print(f"{target}: {digest}")
