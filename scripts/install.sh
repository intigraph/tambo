#!/bin/sh
set -eu

# This compatibility URL also redirects after the repository rename.
repo=${TAMBO_REPOSITORY:-${CABOODLE_REPOSITORY:-scbrown/caboodle}}
version=${TAMBO_VERSION:-${CABOODLE_VERSION:-}}
install_dir=${TAMBO_INSTALL_DIR:-${CABOODLE_INSTALL_DIR:-${CARGO_HOME:-$HOME/.cargo}/bin}}

if [ -z "$version" ]; then
    version=$(curl -fsSL --connect-timeout 10 --max-time 120 "https://api.github.com/repos/$repo/releases/latest" |
        sed -n 's/.*"tag_name":[[:space:]]*"\([^"]*\)".*/\1/p' | head -n 1)
fi
case "$version" in
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
if command -v sha256sum >/dev/null; then
    (cd "$work" && sha256sum -c "$archive.sha256")
else
    expected=$(awk '{print $1}' "$work/$archive.sha256")
    actual=$(shasum -a 256 "$work/$archive" | awk '{print $1}')
    test "$actual" = "$expected" || {
        printf '%s\n' "$archive: checksum mismatch" >&2
        exit 1
    }
fi
tar -xzf "$work/$archive" -C "$work"
for binary in $binaries; do
    test -x "$work/$binary"
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
