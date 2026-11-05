#!/bin/sh
# arming: ci Documentation workflow via check-docs.sh
# Check our own main-branch file links against this checkout, not GitHub HTML.
# This remaps their destination; it does not exclude them or accept errors.
set -eu
cd "$(dirname "$0")/.."
repo_uri=$(python3 -c 'from pathlib import Path; print(Path.cwd().as_uri())')
# check-docs.sh supplies the fresh build root. Keep the original argv intact.
root_dir=
expect_root=false
for arg do
  if [ "$expect_root" = true ]; then
    root_dir=$arg
    expect_root=false
  else
    case "$arg" in
      --root-dir) expect_root=true ;;
      --root-dir=*) root_dir=${arg#--root-dir=} ;;
    esac
  fi
done
if [ "$expect_root" = true ]; then
  echo 'Missing --root-dir value' >&2
  exit 2
fi
if [ -n "$root_dir" ]; then
  book_uri=$(python3 -c 'from pathlib import Path; import sys; print((Path(sys.argv[1]).resolve() / "tambo").as_uri())' "$root_dir")
  set -- --remap "^https://intigraph\\.github\\.io/tambo/ ${book_uri}/" "$@"
  set -- --remap "^https://scbrown\\.github\\.io/(caboodle|tambo)/ ${book_uri}/" "$@"
fi
exec lychee --config .lychee.toml \
  --remap "^https://github\\.com/intigraph/tambo/blob/main/ ${repo_uri}/" \
  --remap "^https://github\\.com/scbrown/(caboodle|tambo)/blob/main/ ${repo_uri}/" "$@"
