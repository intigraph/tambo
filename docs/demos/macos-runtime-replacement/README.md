# Finite replacement demonstration

Trigger: a different qualified executable meets an occupied create-only tool
destination. The actual base installer refuses it. The candidate transaction
then verifies both old backup copies, changes only the fixture tool/runtime,
restores the old versions through its inverse, retains the new versions, and
refuses replay.

This is a readable **transcript**, not a recording: no terminal recorder is
installed in the author environment. The actual output is retained in
[transcript.txt](transcript.txt). Reproduce from this PR's source:

```sh
python3 scripts/demo-macos-e579-runtime-transaction.py
```

Base is `0f291ca0839d93db8dc8ee8891d7cda1d7c0111f`; the base controller
module's actual create-only `install` function is used, with protected-path
checks redirected only inside a disposable fixture. The candidate source
SHA256 is printed in the transcript; it identifies the exact helper bytes
demonstrated even before publication of the PR commit. Inputs are synthetic
public bytes, without privileged commands or system paths. No native Mac
execution, root rollback proof, account/key/policy admission, freeze custody
or isolation readiness is demonstrated. Native execution stays held for a
separately admitted operator batch.
