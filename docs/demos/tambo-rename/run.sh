#!/bin/sh
set -eu
printf '%s\n' 'Retained cargo metadata baseline (before the rename):'
cat docs/demos/tambo-rename/before-targets.json
printf '\n%s\n' '$ cargo metadata --locked --no-deps --format-version 1 (package and binaries)'
cargo metadata --locked --no-deps --format-version 1 | python3 -c 'import json,sys; p=json.load(sys.stdin)["packages"][0]; print(json.dumps({"package":p["name"],"version":p["version"],"binaries":[t["name"] for t in p["targets"] if "bin" in t["kind"]]},indent=2))'
target=$(cargo metadata --locked --no-deps --format-version 1 | python3 -c 'import json,sys; print(json.load(sys.stdin)["target_directory"])')
printf '\n%s\n' '$ tambo --version (source candidate)'
"$target/release/tambo" --version
printf '\n%s\n' '$ caboodle --version (candidate compatibility entry point)'
"$target/release/caboodle" --version
