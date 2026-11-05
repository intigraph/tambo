# Controller runtime source demonstration

The unchanged provisioning-only manifest on base
`2d48587e64c11b038ad6fc90d3ff0026a0bee3ba` lacks eight fields required by the
pinned job launcher. The candidate's explicit migration adds those eight fields
while preserving the original eight identity fields' bytes; adding a new UID
instead refuses. This uses public synthetic inputs and makes no host changes.

[Replay output](output.txt) and [timing](timing.txt) are an output-only terminal
recording, not an inline video. Download both into one directory and run
`scriptreplay --log-out output.txt --log-timing timing.txt` with util-linux.
Reproduce from the containing source revision with
`python3 -B docs/demos/controller-runtime/demo.py`.

The recording belongs to the same source revision as this caption; its relevant
function and retained base fixture are included in that revision. Private ledger
binding, root filesystem migration, native account isolation, Mac imports and
actual lifecycle execution are outside this demonstration. CI controls cover
private synthetic ledger/filesystem cases. Native readiness stays false, pending
the separately tracked source dependencies and operator acceptance.

The small sanitized output/timing pair is retained with this operator-packet
example in the repository's existing docs/demos destination.
