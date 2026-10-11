#!/bin/sh
set -eu
test -n "$LYCHEE"
root=$(pwd)
fixture=$(mktemp -d /tmp/caboodle-link-demo.XXXXXX)
trap 'rm -rf "$fixture"' EXIT
mkdir -p "$fixture/scripts" "$fixture/skills/caboodle"
cp scripts/check-doc-links.sh "$fixture/scripts/"
cp .lychee.toml "$fixture/"
cp skills/caboodle/SKILL.md "$fixture/skills/caboodle/"
printf '%s\n' '[skill](https://github.com/scbrown/caboodle/blob/main/skills/caboodle/SKILL.md)' > "$fixture/input.md"
unset GH_TOKEN GITHUB_TOKEN
export PATH="$(dirname "$LYCHEE"):/usr/bin:/bin"
cd "$fixture"
printf '%s\n' 'BEFORE: normal GitHub HTML request; current availability sample'
set +e
"$LYCHEE" --config .lychee.toml --max-retries 0 --timeout 4 input.md
before=$?
set -e
printf 'Before exit: %s\n' "$before"
printf '%s\n' 'AFTER: identical Markdown input, checkout destination'
sh scripts/check-doc-links.sh --offline input.md
printf '%s\n' 'NEGATIVE CONTROL: remove local target; identical input must fail'
rm skills/caboodle/SKILL.md
set +e
sh scripts/check-doc-links.sh --offline input.md
missing=$?
set -e
printf 'Missing-target exit: %s\n' "$missing"
test "$missing" -ne 0
