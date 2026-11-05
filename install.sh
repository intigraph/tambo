#!/bin/sh
set -eu

# Canonical repository; retain both Tambo and legacy Caboodle overrides.
repo=${TAMBO_REPOSITORY:-${CABOODLE_REPOSITORY:-intigraph/tambo}}
version=${TAMBO_VERSION:-${CABOODLE_VERSION:-}}
install_dir=${TAMBO_INSTALL_DIR:-${CABOODLE_INSTALL_DIR:-${CARGO_HOME:-$HOME/.cargo}/bin}}

if [ -z "$version" ]; then
    version=$(curl -fsSL --connect-timeout 10 --max-time 120 "https://api.github.com/repos/$repo/releases/latest" |
        sed -n 's/.*"tag_name":[[:space:]]*"\([^"]*\)".*/\1/p' | head -n 1)
fi
case "$version" in
    v[0-9]*[!A-Za-z0-9._-]*) printf '%s\n' "invalid release tag" >&2; exit 1 ;;
    v[0-9]*) ;;
    *) printf '%s\n' "invalid or unavailable Tambo release version: $version" >&2; exit 1 ;;
esac

if [ -n "${TAMBO_TARGET:-${CABOODLE_TARGET:-}}" ]; then
    target=${TAMBO_TARGET:-$CABOODLE_TARGET}
else
    machine=$(uname -m)
    system=$(uname -s)
    case "$system:$machine" in
        Linux:x86_64) target=x86_64-unknown-linux-gnu ;;
        Darwin:x86_64) target=x86_64-apple-darwin ;;
        Darwin:arm64) target=aarch64-apple-darwin ;;
        *) printf '%s\n' "unsupported Tambo release target: $system $machine" >&2; exit 1 ;;
    esac
fi

case "$target" in
    x86_64-unknown-linux-gnu|x86_64-apple-darwin|aarch64-apple-darwin) ;;
    *) printf '%s\n' "unsupported Tambo release target: $target" >&2; exit 1 ;;
esac

archive="tambo-$version-$target.tar.gz"
binaries='tambo caboodle'
base=${TAMBO_RELEASE_BASE_URL:-${CABOODLE_RELEASE_BASE_URL:-https://github.com/$repo/releases/download/$version}}
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT HUP INT TERM

if ! status=$(curl -sSL --connect-timeout 10 --max-time 120 \
    -w '%{http_code}' "$base/$archive" -o "$work/$archive"); then
    printf '%s\n' 'release archive transfer failed' >&2
    exit 1
fi
case "$status" in
    200) ;;
    000)
        # Local release fixtures use file://, which has no HTTP status.
        case "$base" in file://*) ;; *) exit 1 ;; esac
        ;;
    404)
        archive="caboodle-$version-$target.tar.gz"
        binaries=caboodle
        curl -fsSL --connect-timeout 10 --max-time 120 "$base/$archive" -o "$work/$archive"
        ;;
    *) printf '%s\n' "release archive unavailable (HTTP $status)" >&2; exit 1 ;;
esac
curl -fsSL --connect-timeout 10 --max-time 120 "$base/$archive.sha256" -o "$work/$archive.sha256"
python3 - "$work/$archive.sha256" "$archive" <<'PYCHECKSUM'
import pathlib, re, sys
fields = pathlib.Path(sys.argv[1]).read_text().split()
if (len(fields) != 2 or not re.fullmatch(r"[0-9a-fA-F]{64}", fields[0])
        or fields[1].lstrip("*") != sys.argv[2]):
    raise SystemExit("invalid release checksum manifest")
PYCHECKSUM
expected=$(awk '{print $1}' "$work/$archive.sha256" | tr 'A-F' 'a-f')
if command -v sha256sum >/dev/null; then
    actual=$(sha256sum "$work/$archive" | awk '{print $1}')
else
    actual=$(shasum -a 256 "$work/$archive" | awk '{print $1}')
fi
test "$actual" = "$expected" || {
    printf '%s\n' "$archive: checksum mismatch" >&2
    exit 1
}
# Only the declared regular executables may leave the verified archive.
python3 - "$work" "$archive" $binaries <<'PYARCHIVE'
import pathlib, sys, tarfile
root, name = pathlib.Path(sys.argv[1]), sys.argv[2]
expected = set(sys.argv[3:])
with tarfile.open(root / name, "r:gz") as bundle:
    members = bundle.getmembers()
    if len(members) != len(expected) or {m.name for m in members} != expected:
        raise SystemExit("unexpected release archive members")
    for member in members:
        if not member.isfile() or member.size > 256 * 1024 * 1024:
            raise SystemExit("invalid release executable")
        source = bundle.extractfile(member)
        if source is None:
            raise SystemExit("missing release executable")
        (root / member.name).write_bytes(source.read())
        (root / member.name).chmod(0o755)
PYARCHIVE
for binary in $binaries; do
    test -f "$work/$binary" && test ! -L "$work/$binary" && test -x "$work/$binary"
    reported=$("$work/$binary" --version)
    case "$reported" in
        "$binary ${version#v}"|"$binary ${version#v} "*) ;;
        *) printf '%s\n' "release version mismatch: $binary" >&2; exit 1 ;;
    esac
done
mkdir -p "$install_dir"
for binary in $binaries; do
    staged="$install_dir/.$binary.$$.tmp"
    cp "$work/$binary" "$staged"
    chmod 0755 "$staged"
    mv "$staged" "$install_dir/$binary"
done
for binary in $binaries; do
    "$install_dir/$binary" --version
    printf '%s\n' "installed $install_dir/$binary"
done

# Name a stale executable for either command; version equality cannot prove
# that PATH resolves to the binary just installed.
for binary in $binaries; do
    resolved=$(command -v "$binary" 2>/dev/null || true)
    if [ -n "$resolved" ] && ! [ "$resolved" -ef "$install_dir/$binary" ]; then
        printf '%s\n' "WARNING: \`$binary\` on PATH is $resolved ($("$resolved" --version 2>&1 | head -n 1)), not the copy just installed at $install_dir/$binary. Remove the stale copy or put $install_dir earlier on PATH." >&2
    elif [ -z "$resolved" ]; then
        printf '%s\n' "note: $install_dir is not on PATH; add it to run \`$binary\`" >&2
    fi
done
