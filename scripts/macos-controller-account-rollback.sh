#!/bin/sh
# Account-only rollback. Revoke later runtime/key grants via their reviewed packet first.
set -eu
HERE=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
exec /usr/bin/python3 -I "$HERE/macos-controller-account-packet.py" rollback "$@"
