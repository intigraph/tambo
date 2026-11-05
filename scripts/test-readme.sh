#!/bin/bash
# arming: ci Documentation workflow; also runnable locally
# Execute the README's release install and first-success blocks with no inherited
# tool installations or configuration. Require both documented functional checks.
set -euo pipefail
repo=$(cd "$(dirname "$0")/.." && pwd)
work=$(mktemp -d)
cleanup() { rm -rf "$work"; }
trap cleanup EXIT
mkdir "$work/home" "$work/run"
# Use the checkout's checksummed release-shaped fixture so new documented
# behavior is tested before release, not against an older published binary.
cargo build --release --locked --manifest-path "$repo/Cargo.toml"
version=$(sed -n 's/^version = "\([^"]*\)"$/\1/p' "$repo/Cargo.toml" | head -n 1)
target_dir=$(cargo metadata --no-deps --format-version 1 --manifest-path "$repo/Cargo.toml" | python3 -c 'import json,sys; print(json.load(sys.stdin)["target_directory"])')
archive="tambo-v$version-x86_64-unknown-linux-gnu.tar.gz"
mkdir "$work/release"
tar -czf "$work/release/$archive" -C "$target_dir/release" tambo caboodle
(cd "$work/release" && sha256sum "$archive" > "$archive.sha256")
# Extract the actual README commands; no issued-token fixture or injected
# provisioning commands are allowed on this default-path proof.
python3 - "$repo/README.md" "$work/commands.sh" <<'PYEXTRACT'
import pathlib, re, sys
text = pathlib.Path(sys.argv[1]).read_text()
install = text.split("## Install", 1)[1].split("## First success", 1)[0]
local = text.split("## First success", 1)[1].split("### Advanced:", 1)[0]
bootstrap = re.findall(r"```bash\n(.*?)```", install, re.S)[0]
# Exercise this proposal's installer rather than fetching unrelated main bytes.
bootstrap = bootstrap.replace('https://raw.githubusercontent.com/intigraph/tambo/main/install.sh', pathlib.Path(sys.argv[1]).parent.joinpath('install.sh').as_uri())
commands = re.findall(r"```bash\n(.*?)```", local, re.S)
assert commands == ["tambo quickstart\n", "tambo verify\n"], commands
pathlib.Path(sys.argv[2]).write_text(bootstrap + '\nexport PATH="${CARGO_HOME:-$HOME/.cargo}/bin:$PATH"\n' + "\n".join(commands))
PYEXTRACT
(cd "$work/run" && env -i HOME="$work/home" PATH=/usr/bin:/bin \
  CABOODLE_VERSION="v$version" CABOODLE_RELEASE_BASE_URL="file://$work/release" \
  /bin/bash --noprofile --norc -e "$work/commands.sh") > "$work/actual.txt"
for tool in quipu camayoc bobbin; do
  test "$(grep -c "^$tool: verified$" "$work/actual.txt")" -eq 2
done
test ! -e "$work/home/.config/quipu/token"
if grep -q '^quipu credential:' "$work/actual.txt"; then
  printf '%s\n' 'local default unexpectedly required a credential' >&2
  exit 1
fi
printf '%s\n' 'README local quickstart + verify: passed in a fresh HOME without issued token or server'
