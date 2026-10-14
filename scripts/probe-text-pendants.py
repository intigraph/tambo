#!/usr/bin/env python3
"""Repeat the source-only before/after format measurement; never opens a live DB."""
import json
from pathlib import Path
import sqlite3
import subprocess
import tempfile

root=Path(__file__).resolve().parents[1]
base="8cec4fd3e07c1b3da98563879c1ecbb6a6ecdff5"
names=("stack-map","stack-operations")
print("BASE",base)
with tempfile.TemporaryDirectory() as work:
    for name in names:
        data=subprocess.check_output(["git","-C",str(root),"show",f"{base}:packs/{name}.pendant.db"])
        path=Path(work)/(name+".db");path.write_bytes(data)
        with sqlite3.connect("file:"+str(path)+"?mode=ro&immutable=1",uri=True) as db:
            count=db.execute("SELECT COUNT(*) FROM facts").fetchone()[0]
        print(name,"SQLite payload bytes",len(data),"facts",count)
print("CANDIDATE text directories")
for name in names:
    path=root/"packs"/name/"pendant"
    manifest=json.loads((path/"manifest.json").read_text())
    print(name,"members",len(list(path.iterdir())),"triples",len((path/"export.nt").read_text().splitlines()),"graph_hash",manifest["graph_hash"])
    print("legacy directory aliases",all((root/"packs"/f"{name}.{suffix}").resolve()==path.resolve() for suffix in ("pendant.db","qpack.db")))
print("LIMIT: source format/graph counts only; native admission, promotion and deployment are separate.")
