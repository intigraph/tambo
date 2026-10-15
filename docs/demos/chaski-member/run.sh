#!/usr/bin/env bash
set -euo pipefail
python3 - <<'PYCODE'
import json,subprocess,tomllib
from pathlib import Path
base="47389353fe7f"
paths=subprocess.check_output(["git","ls-tree","--name-only",base,"members/"],text=True).splitlines()
base_members=[tomllib.loads(subprocess.check_output(["git","show",f"{base}:{p}"],text=True)) for p in paths if p.endswith(".toml")]
head_members=[tomllib.loads(p.read_text()) for p in Path("members").glob("*.toml")]
for label,rows in [("base",base_members),("candidate",head_members)]:
 print(label,json.dumps({"chaski":sum(x["name"]=="chaski" for x in rows),"shuttle_control":sum(x["name"]=="shuttle" for x in rows)},sort_keys=True))
m=next(x for x in head_members if x["name"]=="chaski")
assert m["version"]=="0.1.0" and len(m["programs"])==3
assert m["sha256"]["any"]=="758f08de2a26d093fd5c234205f220a70312f80c459d622ed68ff6a946081b56"
print("pinned",m["version"],*[p["name"] for p in m["programs"]])
print("reader requires",m["verify"][-1]["present"])
PYCODE
