#!/bin/sh
# Explicit supplied binaries; disposable native fixtures only.
set -eu
baseline=$1
candidate=$2
quipu=$3
shapes=$4
printf 'BASELINE installed CLI\n'
"$baseline" --version
set +e
"$baseline" skills --help
status=$?
set -e
printf 'baseline skills exit=%s (expected 2: command absent)\n' "$status"
test "$status" -eq 2
printf '\nCANDIDATE source CLI\n'
"$candidate" --version
python3 tests/test_skills_native.py --tambo "$candidate" --quipu "$quipu" --render-shapes "$shapes" --demo
