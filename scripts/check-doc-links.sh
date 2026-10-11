#!/bin/sh
# arming: ci Documentation workflow via check-docs.sh
# Check our own main-branch file links against this checkout, not GitHub HTML.
# This remaps their destination; it does not exclude them or accept errors.
set -eu
cd "$(dirname "$0")/.."
repo_uri=$(python3 -c 'from pathlib import Path; print(Path.cwd().as_uri())')
exec lychee --config .lychee.toml \
  --remap "^https://github\\.com/scbrown/caboodle/blob/main/ ${repo_uri}/" "$@"
