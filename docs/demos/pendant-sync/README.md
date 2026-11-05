# Repository pendant sync

Target: isolated temporary Git repositories and SQLite stores, using a compiled
baseline and candidate CLI plus the actual local Quipu CLI. This does not install
a binary, load a production store, or arm hooks in a user's checkout.

With the same sync invocation, baseline `a6d7fee6bf59b452427a5b7c4daeb76f54743e82`
rejects the missing `sync` subcommand and creates no consumer database. The
candidate composes a trusted fixture, reports an unchanged repeat, and moves
CURRENT when its pack changes. Queries through `FROM` confirm CURRENT contains
the new label and PREVIOUS retains the old one. A tampered archive fails native
hash verification while the receipt, database bytes and CURRENT remain unchanged.
Removing the pack removes its content from CURRENT; ROOT remains empty.

Reproduce from the repository root with separately built executables:

```bash
BASELINE_BINARY=/path/to/baseline/tambo \
CANDIDATE_BINARY=/path/to/candidate/tambo \
python3 docs/demos/pendant-sync/demo.py
```

The probe requires a Quipu CLI with repository CURRENT composition (the local
recording used clean Quipu `abb9e5a1`, version 0.14.1). The baseline was built
from an archive of the exact base revision; its embedded Git identity is absent
because the archive has no `.git` directory. Archive provenance, rather than
that version field, identifies this baseline. The candidate's source revision
is the commit containing this demo.

[Terminal recording](output.log) and [timing](timing.log) retain real output and
timing from util-linux `script`. Download both and replay with
`scriptreplay --timing timing.log --typescript output.log`. This is a terminal
recording, not a browser video; responses are summarized by the assertion-bearing
probe. The script is the replayable input, and the transcript is readable alone.

Separate CLI controls use a private Git checkout event to exercise opt-in hooks;
the recording demonstrates native composition and refusal, not installed hook
acceptance or complete clone/cloud adoption. An environment setup run that occurs
before checkout still requires sync after the repository becomes available.
