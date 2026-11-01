#!/bin/sh
# Run with sudo from the same reviewed checkout as the setup packet.
set -eu
HERE=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
exec /usr/bin/python3 -I "$HERE/macos-job-account-packet.py" rollback
