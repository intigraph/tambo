# Native skill publication and whole-profile staging

The recorded installed baseline is Tambo `0.13.4 (48ac88ea4fd8)`: its actual
`skills --help` call exits 2 because that command is absent. This is the
installed historical baseline, not a rebuilt binary of PR base `d6e809b`.

The candidate runtime source is exact commit
`b3ee8b8f00e0050e6809f3862b6f286e22555bf2`. It publishes two public pendants and
one private pendant, then stages the three selected skills under both harness
layouts. Watch the actual names, literal template-like markdown, executable
mode, nested metadata file and native NOOP receipt. The real source-update
control republishes version 2, selects its new pin, and observes the new
description in both trees while unchanged file inode/mtime/bytes remain equal.

The driver also exercises incomplete metadata, wrong pins, public/internal
conflicts, corrupt archives, immutable publication output, forged public
metadata, unknown destination files and owned-only retirement. All assertions
run against actual native Quipu `0.17.0` source
`f3982337b2211a28b8cf465faebad23ca51b28ee` and reviewed render shapes.
These are disposable local fixtures. IRIs, share IDs and catalogue clocks are
synthetic inputs, not production bindings. No existing harness, catalogue,
server store or settings file is changed. Staged directories are not fresh
Claude/Codex session evidence or authenticated loaded-version history.

The final recording/test commit leaves the candidate runtime unchanged;
[source hashes](source-hashes.json) bind its runtime files. The small recording
is retained in this repository. Its command header omits private paths; real
terminal output and timing are preserved.

Replay with util-linux (download the files together):

```sh
scriptreplay --log-out docs/demos/skill-profile/output.log --log-timing docs/demos/skill-profile/timing.log
```

[Readable terminal transcript](output.log). To repeat, from the repository root:

```sh
sh docs/demos/skill-profile/run.sh /owned/baseline-tambo /owned/candidate-tambo /owned/quipu /reviewed/render.ttl
```

The supplied baseline must lack `skills`; the script checks its observed exit
status. The native driver creates and removes temporary stores and never
downloads or installs tools. This supports source review, not live rollout.
