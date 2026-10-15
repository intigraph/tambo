#!/bin/sh
# Exit 0: exact visible version; 1: explicit missing version; 2: UNKNOWN.
set -eu
crate=${1:?crate name required}
version=${2:?stable version required}
case "$crate" in ''|*[!a-zA-Z0-9_-]*) exit 2 ;; esac
case "$version" in ''|*[!0-9.]*) exit 2 ;; esac
python3 - "$version" <<'PYVERSION'
import re, sys
sys.exit(0 if re.fullmatch(r"(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)", sys.argv[1]) else 2)
PYVERSION
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT HUP INT TERM
base=${TAMBO_REGISTRY_API:-https://crates.io/api/v1}
if ! status=$(curl -sS --max-time 20 --max-filesize 1048576 \
    -H 'User-Agent: tambo-release-verifier' -o "$work/result.json" \
    -w '%{http_code}' "$base/crates/$crate/$version"); then
    exit 2
fi
case "$status" in
    404) exit 1 ;;
    200) ;;
    *) printf '%s\n' "registry response UNKNOWN (HTTP $status)" >&2; exit 2 ;;
esac
python3 - "$work/result.json" "$crate" "$version" <<'PY'
import json, sys
try:
    row = json.load(open(sys.argv[1]))["version"]
    valid = row["crate"] == sys.argv[2] and row["num"] == sys.argv[3] and row["yanked"] is False
except (OSError, ValueError, KeyError, TypeError):
    valid = False
sys.exit(0 if valid else 2)
PY
