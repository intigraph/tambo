# Homebrew Quipu with a stale Cargo installation

The real installer CLI runs against disposable Homebrew and tool fixtures. Quipu on PATH and its package receipt are 0.14.1; a separate Cargo pair is 0.8.1. No remote service or real package manager is used.

Published Tambo 0.3.5 fails with a receipt/executable mismatch before Bobbin delivery. The candidate selects the package PATH pair and upgrades the fixture Bobbin from 0.28.0 to 0.29.0. Both paths refresh metadata once. Matching receipt checks remain required for both Quipu client and server.

Replay with verified old and candidate binaries:

```sh
python3 demo.py /path/to/official-0.3.5/tambo /path/to/candidate/tambo
```

The terminal recording is [output.log](output.log), with [timing.log](timing.log). Replay from this directory with `scriptreplay timing.log output.log`.

This is an actual CLI fixture demonstration, not installed Mac acceptance. The Mac must complete its next natural run after the normal published release.
