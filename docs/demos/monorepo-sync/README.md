# Native monorepo pendant demonstration

This is a replayable terminal recording of isolated source behavior. A private
fixture uses the package layout from Reckoning commit
`e69450e4a449df74cd6b989afa4704b17fc8b977`. That revision has no CODEOWNERS;
the users and team rules in the demonstration are explicit synthetic inputs.
No repository installation, server mutation, ROOT promotion or live hook arming
is performed. Git hooks run only in the temporary fixture.

Before: Tambo source `7f447974e595d2d4fe1d12d62ead5e2c66b7e3b9` rejects the same
`packs project-scope` command. Its implementation is unchanged by integration
base `6344cb35b6436896b75b414792638bb3335b7b3d` (the base update adds README docs).
After: candidate implementation `5d54c158db95945b8a9e8e38d181bbca1b33add1`
prints a native path scope and composes three project shares while excluding
the parent. Native Quipu is `0.14.1`, clean `cb0350800ebf8f5dffae1a8ac994159ea552762f`.
The source inventory below identifies the implementation independently of build
metadata and later caption/recording commits.

Watch these controls in order:

- An unknown CODEOWNERS resource refuses; registered resources yield IRI ownership.
- Flat-mode receipts remain readable by the baseline binary.
- Only the changed client's pack graph identity changes; server/shared identities remain equal.
- A real Git checkout hook retains monorepo mode.
- Actual sparse checkout changes membership from three projects to two.
- Cross-project edges survive; ROOT stays empty.
- Corrupting the excluded server share refuses while retaining CURRENT and its receipt.

Download [the output](output.log) and [its timing](timing.log), then replay:

```sh
scriptreplay --log-out output.log --log-timing timing.log
```

Playback was verified with util-linux `scriptreplay`. This is a terminal
recording, not an inline video. The readable output is the text fallback.

To reproduce, build Tambo from the candidate source, check out the pinned
Reckoning revision in a separate local directory, then run from Tambo's root:

```sh
CANDIDATE_BINARY=/path/to/built/tambo \
RECKONING_FIXTURE=/path/to/pinned/reckoning \
BASELINE_BINARY=/path/to/baseline/tambo \
python3 docs/demos/monorepo-sync/demo.py
```

`BASELINE_BINARY` is optional; without it the baseline refusal and v1 backward
reader control are omitted. The script creates its own producer/consumer stores,
trust file, shape authority, archives and Git repository. `RECKONING_FIXTURE`
is read only. The fixture source path and ownership predicates are test URNs;
they are not a proposed production schema.

Limits: unchanged members are reused in the consumer, but changed monorepo
compositions also verify every archive in a temporary store. This proof makes
no fleet performance or installed-acceptance claim. Source delivery still needs
CI, independent review and the normal release/adoption process. Before enabling
monorepo mode on an existing installation, retain the prior database and receipt
for rollback; an old binary rejects the new monorepo receipt format.
