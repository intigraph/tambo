#!/bin/sh
#
# Rebuild the stack knowledge packs in packs/ from their Turtle sources in
# packs/src/.
#
# Each packs/<name>/pendant directory is a standard inspectable text share.
# Scratch SQLite stores are private build inputs and are never checked in.
# Consumers use `quipu import` on the directory or its published archive.
#
# Not byte-reproducible across runs: quipu stamps the pack with its creation
# source identity, so a rebuild with identical sources can change its manifest. That
# is why this script is run by hand when packs/src/ changes, not by CI on
# every push — a hash that churns without a source change is noise dressed as
# a diff.
#
# Requires a quipu binary (QUIPU=... overrides discovery).
set -eu

quipu=${QUIPU:-quipu}
command -v "$quipu" >/dev/null 2>&1 || {
    printf '%s\n' "quipu binary not found (set QUIPU=/path/to/quipu)" >&2
    exit 1
}

root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
out_dir=${PACKS_OUTPUT_DIR:-"$root/packs"}
src_dir="$root/packs/src"
mkdir -p "$out_dir"
work=$(mktemp -d "$out_dir/.stack-pendants.XXXXXX")
trap 'rm -rf "$work"' EXIT HUP INT TERM

for name in stack-map stack-operations; do
    target="$out_dir/$name/pendant"
    if [ -L "$target" ] || { [ -e "$target" ] && [ ! -d "$target" ]; }; then
        printf '%s\n' "refusing non-directory output: $target" >&2
        exit 1
    fi
done

# Outward packs are refused unless a block-tier identifier-policy catalogue is
# loaded in the producing store (quipu >= 0.9.1). The catalogue lives in its own
# named graph, so it gates the pack without travelling in it. Idempotent: the
# assembly store is shared by both packs.
policy_dir="$root/packs/policy"
load_identifier_policy() {
    "$quipu" shapes load identifier-policy "$policy_dir/identifier-policy.shapes.ttl" --db "$1" >/dev/null
    "$quipu" knot "$policy_dir/identifier-policy.ttl" --graph urn:caboodle:identifier-policy --db "$1" >/dev/null
}

build_pack() {
    name=$1
    graph="https://caboodle.dev/graph/$name"
    src="$src_dir/$name.ttl"
    out="$work/$name-pendant"
    test -f "$src" || { printf '%s\n' "missing source: $src" >&2; exit 1; }

    # knot writes to the scratch store's ROOT; graph import lifts that ROOT
    # into a named graph in the assembly store, which is what pack exports.
    "$quipu" knot "$src" --db "$work/$name-scratch.db"
    "$quipu" graph import "$work/$name-scratch.db" --as "$graph" --db "$work/asm.db"
    load_identifier_policy "$work/asm.db"
    "$quipu" share --graph "$graph" --output "$out" --pack-dir pendant \
        --no-shapes --no-queries --db "$work/asm.db"
    # Native import verifies hashes and bundle identity before accepting data.
    # This is an isolated scratch store, never an installed consumer.
    "$quipu" import "$out" --db "$work/$name-verify.db" > "$work/$name-verification.json"
    python3 - "$out/manifest.json" "$work/$name-verification.json" <<'PY'
import json, sys
manifest, result = [json.load(open(path)) for path in sys.argv[1:]]
assert result['share_id'] == manifest['share_id']
assert result['graph_hash'] == manifest['graph_hash']
assert result['outcome'] in ('staged', 'quarantined')
assert result['attestation']['tier'] == 'transport'
print('Bundle hashes verified; receiver outcome:', result['outcome'])
# Quarantine is preserved: integrity is not vocabulary or promotion approval.
PY
}

build_pack stack-map
build_pack stack-operations

# Publish only after BOTH bundles verify. Existing output survives a failed
# assembly or verification; each replacement uses same-filesystem renames.
for name in stack-map stack-operations; do
    parent="$out_dir/$name"
    target="$parent/pendant"
    backup="$work/$name-old"
    mkdir -p "$parent"
    if [ -L "$target" ] || { [ -e "$target" ] && [ ! -d "$target" ]; }; then
        printf '%s\n' "refusing non-directory output: $target" >&2
        exit 1
    fi
    if [ -d "$target" ]; then mv "$target" "$backup"; fi
    if ! mv "$work/$name-pendant" "$target"; then
        if [ -d "$backup" ]; then mv "$backup" "$target"; fi
        exit 1
    fi
done

printf '%s\n' "text pendants rebuilt in $out_dir"
