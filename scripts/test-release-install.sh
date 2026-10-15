#!/bin/sh
set -eu

repo_root=$(CDPATH= cd -- "$(dirname "$0")/.." && pwd)
version=$(sed -n 's/^version = "\([^"]*\)"$/\1/p' "$repo_root/Cargo.toml" | head -n 1)
target=x86_64-unknown-linux-gnu
fixture=$(mktemp -d)
trap 'rm -rf "$fixture"' EXIT HUP INT TERM
archive="tambo-v$version-$target.tar.gz"

cargo build --release --locked --manifest-path "$repo_root/Cargo.toml"
target_dir=$(cargo metadata --format-version 1 --no-deps --manifest-path "$repo_root/Cargo.toml" |
    python3 -c 'import json, sys; print(json.load(sys.stdin)["target_directory"])')
# Cargo may route artifacts outside the source tree. Test an owned copy rather
# than assuming target/release or retaining a mutable build-output alias.
mkdir -p "$fixture/program"
cp "$target_dir/release/tambo" "$fixture/program/tambo"
cp "$target_dir/release/caboodle" "$fixture/program/caboodle"
tar -czf "$fixture/$archive" -C "$fixture/program" tambo caboodle
(cd "$fixture" && sha256sum "$archive" > "$archive.sha256")
legacy="caboodle-v$version-$target.tar.gz"
cp "$fixture/$archive" "$fixture/$legacy"
(cd "$fixture" && sha256sum "$legacy" > "$legacy.sha256")
(cd "$repo_root" && scripts/check-release-assets.sh "v$version" "$fixture")

CABOODLE_VERSION="v$version" \
CABOODLE_TARGET="$target" \
CABOODLE_RELEASE_BASE_URL="file://$fixture" \
CABOODLE_INSTALL_DIR="$fixture/bin" \
    "$repo_root/scripts/install.sh"
"$fixture/bin/caboodle" --version | grep -F "caboodle $version"
test -x "$fixture/bin/tambo"
test -x "$fixture/bin/caboodle"

# A stale copy earlier on PATH must be named, and the installed one must not.
mkdir -p "$fixture/stale"
printf '#!/bin/sh\necho "caboodle 0.0.1 (stale)"\n' > "$fixture/stale/caboodle"
chmod 0755 "$fixture/stale/caboodle"
shadow_err=$(PATH="$fixture/stale:$fixture/bin:$PATH" \
    CABOODLE_VERSION="v$version" CABOODLE_TARGET="$target" \
    CABOODLE_RELEASE_BASE_URL="file://$fixture" CABOODLE_INSTALL_DIR="$fixture/bin" \
    "$repo_root/scripts/install.sh" 2>&1 >/dev/null)
printf '%s\n' "$shadow_err" | grep -F "WARNING: \`caboodle\` on PATH is $fixture/stale/caboodle (caboodle 0.0.1 (stale))"
clean_err=$(PATH="$fixture/bin:$fixture/stale:$PATH" \
    CABOODLE_VERSION="v$version" CABOODLE_TARGET="$target" \
    CABOODLE_RELEASE_BASE_URL="file://$fixture" CABOODLE_INSTALL_DIR="$fixture/bin" \
    "$repo_root/scripts/install.sh" 2>&1 >/dev/null)
if printf '%s\n' "$clean_err" | grep -q WARNING; then
    printf '%s\n' 'installer warned although PATH runs the installed copy' >&2
    exit 1
fi

# Both command names must execute the same argument parsing and plan semantics.
"$fixture/bin/tambo" --version > "$fixture/tambo-version"
"$fixture/bin/caboodle" --version > "$fixture/caboodle-version"
cut -d ' ' -f 2- "$fixture/tambo-version" > "$fixture/tambo-identity"
cut -d ' ' -f 2- "$fixture/caboodle-version" > "$fixture/caboodle-identity"
cmp "$fixture/tambo-identity" "$fixture/caboodle-identity"
cat > "$fixture/intent.toml" <<'INTENT'
intended_use = "compare compatible command names"
[[anticipated_questions]]
question = "which entities are present?"
answer_shape = "entity list"
seed_intent = "synthetic entity fixture"
sparql = "SELECT ?s WHERE { ?s ?p ?o }"
expected = "fixture-result"
INTENT
"$fixture/bin/tambo" plan --intent "$fixture/intent.toml" --profile retrieval --output "$fixture/tambo-plan.toml"
"$fixture/bin/caboodle" plan --intent "$fixture/intent.toml" --profile retrieval --output "$fixture/caboodle-plan.toml"
cmp "$fixture/tambo-plan.toml" "$fixture/caboodle-plan.toml"
for binary in tambo caboodle; do
    if "$fixture/bin/$binary" not-a-command >/dev/null 2>&1; then
        printf '%s\n' "$binary accepted an unknown command" >&2
        exit 1
    fi
done

# An incomplete but correctly checksummed archive must not replace either file.
sha256sum "$fixture/bin/tambo" "$fixture/bin/caboodle" > "$fixture/installed-before"
cp "$fixture/$archive" "$fixture/complete.tar.gz"
tar -czf "$fixture/$archive" -C "$fixture/program" tambo
(cd "$fixture" && sha256sum "$archive" > "$archive.sha256")
if CABOODLE_VERSION="v$version" CABOODLE_TARGET="$target" \
   CABOODLE_RELEASE_BASE_URL="file://$fixture" CABOODLE_INSTALL_DIR="$fixture/bin" \
   "$repo_root/scripts/install.sh" >/dev/null 2>&1; then
    printf '%s\n' 'archive without the legacy entry point unexpectedly installed' >&2
    exit 1
fi
sha256sum -c "$fixture/installed-before"
mv "$fixture/complete.tar.gz" "$fixture/$archive"
(cd "$fixture" && sha256sum "$archive" > "$archive.sha256")

printf 'corrupt' >> "$fixture/$archive"
if CABOODLE_VERSION="v$version" \
   CABOODLE_TARGET="$target" \
   CABOODLE_RELEASE_BASE_URL="file://$fixture" \
   CABOODLE_INSTALL_DIR="$fixture/bin" \
       "$repo_root/scripts/install.sh" >/dev/null 2>&1; then
    printf '%s\n' 'corrupt release archive unexpectedly installed' >&2
    exit 1
fi
test -x "$fixture/bin/caboodle"
rm "$fixture/bin/caboodle"
test ! -e "$fixture/bin/caboodle"
printf '%s\n' 'release installer fixture: verified'
