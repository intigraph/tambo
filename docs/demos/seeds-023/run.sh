#!/usr/bin/env bash
set -euo pipefail
show_pin='import sys,tomllib; p=tomllib.loads(sys.stdin.read()); print("Seeds pin:",p["version"]); print("Targets:",len(p["sha256"])); [print(k,v) for k,v in sorted(p["sha256"].items())]'
printf '%s\n' 'BEFORE: reviewed base 5ca4a12 member manifest'
git show 5ca4a12:members/seeds.toml | python3 -c "$show_pin"
printf '%s\n' 'AFTER: candidate member manifest'
python3 -c "$show_pin" < members/seeds.toml
