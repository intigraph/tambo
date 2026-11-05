# Verified bootstrap demonstration

Previously the bootstrap checked the installed version after replacing files.
It now checks the staged executable first, so a mismatched release preserves
previous bytes. A standalone root entrypoint supports the short README command.

[Recorded fixture run](install-bootstrap.txt) · [timing](install-bootstrap.timing).
Ten controls pass, including valid current/compatibility archives, corrupt
checksum, missing asset, unsupported platform and wrong-version refusal.
These are synthetic local release fixtures, not cross-platform binary proof.

Replay downloaded files with:

```sh
scriptreplay --log-out install-bootstrap.txt --log-timing install-bootstrap.timing
```

Repeat the controls with `python3 scripts/test-install-bootstrap.py`.
Separately, the actual published Linux x86_64 v0.4.0 archive was checksum-verified,
installed in a temporary prefix, and both commands reported version 0.4.0.
No host installation or new release was performed.
