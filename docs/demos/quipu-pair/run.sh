#!/bin/sh
# Metadata-only CLI demonstration. All installed tools and HTTP are fixtures.
set -eu
: "${BEFORE_BINARY:?set the existing baseline Caboodle executable}"
: "${AFTER_BINARY:?set the candidate Caboodle executable}"
root=$(mktemp -d)
trap 'rm -rf "$root"' EXIT HUP INT TERM
mkdir -p "$root/cargo/bin"
for binary in quipu quipu-server; do
  cat > "$root/cargo/bin/$binary" <<PROGRAM
#!/bin/sh
test "\$1" = --version || exit 99
echo '$binary 0.11.1'
PROGRAM
  chmod +x "$root/cargo/bin/$binary"
done
case "$(uname -s)-$(uname -m)" in
  Linux-x86_64) target=x86_64-unknown-linux-gnu;;
  Linux-aarch64) target=aarch64-unknown-linux-gnu;;
  Darwin-x86_64) target=x86_64-apple-darwin;;
  Darwin-arm64) target=aarch64-apple-darwin;;
  *) echo 'unsupported demonstration host' >&2; exit 1;;
esac
archive="quipu-quipu-ai-v0.12.0-$target.tar.gz"
cat > "$root/metadata" <<JSON
{"tag_name":"quipu-ai-v0.12.0","draft":false,"prerelease":false,"assets":[{"name":"$archive"},{"name":"$archive.sha256"}]}
JSON
cat > "$root/cargo/bin/curl" <<'PROGRAM'
#!/bin/sh
for arg do
  case "$arg" in
    https://api.github.com/repos/scbrown/quipu/releases/tags/quipu-ai-v0.12.0)
      cat "$HOME/metadata"; exit 0;;
  esac
done
echo 'unexpected network or artifact download' >&2
exit 99
PROGRAM
chmod +x "$root/cargo/bin/curl"
export HOME="$root" CARGO_HOME="$root/cargo" XDG_CONFIG_HOME="$root/config" XDG_STATE_HOME="$root/state" XDG_CACHE_HOME="$root/cache"
export PATH="$root/cargo/bin:/usr/bin:/bin"
unset QUIPU_AUTH_TOKEN QUIPU_AUTH_TOKEN_FILE CABOODLE_HOLD_FILE
cd "$root"
"$AFTER_BINARY" plan --profile kg > /dev/null
printf 'Same synthetic pair: CLI/server 0.11.1; reviewed publication metadata 0.12.0\n\n'
printf 'BEFORE: '
"$BEFORE_BINARY" --version
printf '$ caboodle update-release --tool quipu --check\n'
if "$BEFORE_BINARY" update-release --tool quipu --check; then
  echo 'baseline unexpectedly admitted Quipu' >&2; exit 1
else printf 'Observed: refused at tool selection\n'; fi
printf '\nAFTER: '
"$AFTER_BINARY" --version
printf '$ caboodle update-release --tool quipu --check\n'
"$AFTER_BINARY" update-release --tool quipu --check
printf '\nControl: no artifact download, backup, state write or executable replacement\n'
test ! -e .caboodle/state.json
test ! -e .caboodle/release-backups
