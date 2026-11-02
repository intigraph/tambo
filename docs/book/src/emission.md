# Emitting to quipu

Tambo is itself a quipu emitter, twice over:

**The box emits.** Every applied and functionally verified tool transition is
written as a redacted episode under `.caboodle/episodes/`. The filename is the
SHA-256 identity of the event content, so rerunning the same transition creates
no duplicate. A failed delivery leaves the exact bytes pending; a later replay
can safely receive Quipu's `unchanged` outcome and then move the file to
`sent/`.

**Work items need no emitter.** Seeds (`sd`) stores work items in Quipu
directly, so their lifecycle is already in the graph.

**Importing a legacy beads export.** `tambo queue-br export.jsonl` projects
each row of a beads (`br`) JSONL export into created, comment, and (when
closed) closure events; re-reading a snapshot is idempotent. Use it only to
bring history from a beads tracker into Quipu; new work is tracked with `sd`.

Flush only after reviewing the queue. The endpoint must be HTTPS (or explicit
localhost HTTP), and the bearer comes only from `QUIPU_AUTH_TOKEN`; it is never
written into the plan, queue, or command line. Tambo first runs a SPARQL
control whose marker must return, then posts files byte-for-byte:

```bash
tambo queue-br br-export.jsonl
QUIPU_AUTH_TOKEN=... tambo flush-episodes --endpoint https://graph.example
```

Created, updated, and unchanged are successful delivery outcomes. A transport
error, malformed response, or missing outcome preserves the identical pending
file instead of guessing whether the write committed.
