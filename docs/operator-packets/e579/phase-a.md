# Phase-A synthetic release refusal: source runbook, native run HELD

This packet answers the operational steps missing from aegis-c01c.5. It is
source preparation for the native operator, not a live execution grant.
`unattended_uid_ready` stays **FALSE**. No key, policy, account, daemon,
readiness or public GitHub change is made by authoring or testing this packet.
Do not improvise a production event or directly edit any Chaski database.

## Preconditions and the unresolved boundary

The approved order is controller -> runtime -> phase-A refusal -> explicit
1b expansion -> metadata -> policy -> ownkeys -> identity. Public CA checks
are not own-key enrollment. The controller daemon stays disabled during phase A.
A source-only key/config packet is [principal-seeds.md](principal-seeds.md).

Before native admission, the owner must attach a private route sheet with:

| Input | Required evidence |
|---|---|
| ingress | exact Camayoc source/hash, host, actor, operational graph, admitted replay spool path and transport |
| release | one approved scratch owner/repo, unique synthetic delivery ID and repository/release node IDs, tag, occurrence time and SHA256 of exact payload |
| route | exact Chaski source/hash and config/hash; only that repo with `release.jobs: [install]`; administrator; `work_items.kind: seeds`; `spawner.kind: st` |
| receiver | actual execution host, UID, st root/config, sd endpoint/graph, and receiver ledger path from that exact config |
| scheduling | the owner of delivery and retries, bounded attempt policy, no concurrent consumer or automatic repeated delivery during evidence capture |
| receipts | operator identity, UTC time, invoked argv, child exit code, emitter request/evidence, live ledger read/snapshot and item readback |

**STOP until the route sheet is complete and admitted.** CT205 is not assumed
to own the GitHub work ledger just because it runs Chaski's general service.
The source deployment inspected for this change did not establish an installed
GitHub receiver/ledger. An example config under a checkout is not a deployed
route. If the receiver actually runs elsewhere, that host's owner takes the
snapshot and supplies it with provenance; the Mac operator does not invent it.

The inspected Chaski source has **no global maximum-attempt setting**:
`github_work.py` records a refusal and returns75; `emitter.py` retries with
backoff. Do not add `max_attempts` to Chaski's routing YAML (unknown fields
refuse) or treat backoff as a finite budget. The approved source design puts
`max_attempts=2` in this synthetic **job's own** private config and uses
`scripts/e579-bounded-delivery.py` for explicit one-shot attempts. It does not
change Chaski defaults, enable a scheduled consumer or prove receiver topology.
The wrapper's third invocation refuses before starting the receiver. Source
controls exercise two child exits75, third-attempt refusal, changed config/pin,
symlink state and interrupted reservation without any native call.

The private job config has exactly these keys (all paths are absolute private
operator inputs, all pins are SHA256 of the actual reviewed bytes):

```json
{
  "version": 1,
  "max_attempts": 2,
  "receiver_script": "/PRIVATE/pinned-chaski/github_work.py",
  "receiver_sha256": "REVIEWED_RECEIVER_SHA256",
  "routing_config": "/PRIVATE/e579/routing.yaml",
  "routing_sha256": "REVIEWED_ROUTING_SHA256",
  "event": "/PRIVATE/e579/emitted-event.json",
  "event_sha256": "REVIEWED_EVENT_SHA256",
  "state": "/PRIVATE/e579/never-reused-budget-state"
}
```

The state path is frozen **inside** that config, with no CLI override. The
native owner verifies all parents/complete pinned Chaski source and an unused
state path, and hashes/retains the config before admission. The wrapper creates
private0700 state and a0600 locked durable reservation before invoking the
receiver; it never edits the receiver ledger. An ambiguous child failure still
consumes its reserved attempt. An interrupted reservation refuses rather than
silently resetting. Preserve all state; a new state path/config is not a
recovery or implicit permission for another attempt. There is no automatic
second delivery. Normal later retry is the second explicit invocation by the
admitted delivery owner after 1b and the remaining qualification gates.

## 1. Produce the GitHub activity through Camayoc ingress

Use an admitted synthetic `release.published`, not a PR: Chaski maps releases
to `install` (`task` on the st side). Do not publish a fake release to a real
project. A private replay delivery has this shape; every fixture field below
must be replaced by the route sheet, and the occurrence time must fall inside
the configured release window:

```json
{
  "headers": {
    "X-GitHub-Event": "release",
    "X-GitHub-Delivery": "e579-fixture-delivery-0001"
  },
  "body": {
    "action": "published",
    "repository": {
      "node_id": "e579-fixture-repository", "id": 90000001,
      "full_name": "example-owner/e579-scratch", "name": "e579-scratch",
      "html_url": "https://github.com/example-owner/e579-scratch",
      "default_branch": "main", "owner": {"login": "example-owner"}
    },
    "release": {
      "node_id": "e579-fixture-release", "id": 90000002,
      "tag_name": "e579-fixture-v1", "draft": false, "prerelease": false,
      "published_at": "2026-11-06T00:00:00Z",
      "html_url": "https://github.com/example-owner/e579-scratch/releases/tag/e579-fixture-v1"
    },
    "sender": {"login": "example-operator"}
  }
}
```

The producer is Camayoc `scripts/github_ingress.py replay`, which reads
`{headers, body}` JSON files, spools by X-GitHub-Delivery and invokes the same
mapper/drain used by ingress. There is no unauthenticated endpoint shortcut.
Use exactly one JSON file in a private directory and a dedicated admitted spool,
not the production poller's spool. First run **without `--post`** and review
the mapped episode. Camayoc's existing Writer uses its admitted authenticated
Quipu configuration; never put a bearer in argv or disable TLS verification.

```sh
python3 "$CAMAYOC_SOURCE/scripts/github_ingress.py" replay "$DELIVERY_DIR" \
  --actor "$INGRESS_ACTOR" --spool "$PREVIEW_SPOOL"
```

The later separately admitted native write uses the same pinned delivery and a
fresh dedicated native spool plus `--post`. A preview marks its own spool as
processed: do not reuse that preview spool and falsely infer a native ingest.
Record both spool identities and the actual ingest result. Read back the typed
GitHubActivity (`release.published`), release/repository identifiers, tag and
occurrence time in the configured operational graph; prove the graph's change
feed includes the transaction. A bare success headline does not prove this hop.

## 2. Select only the install request

The admitted Chaski config names the same exact graph/repo, disables review,
merge and triage for this synthetic route and selects only `release.jobs:
[install]`. A private `--fixture`/JSONL/stub rehearsal is separate from the
native route and cannot supply its work item or refusal evidence.

The source adapter invocation is `chaski-github-routes --config C --quipu Q
--changes`; stdin uses the existing incremental protocol, for example
`{"now": <actual epoch>, "discover": true}`. Use the admitted emitter's actual
protocol input and retain its complete output, not hand-crafted READY evidence.
Verify exactly the intended `install` event. Its key is
`github:install:OWNER/REPO:release/TAG:RELEASE_NODE_LOCAL_NAME`; its request ID
is SHA256 of `chaski.github.work-request/v1\n` plus that exact key. Retain the
actual emitted evidence/event ID and independently recompute it. Another repo,
release or `announce` event is not the same task.

No scheduled emitter or delivery is enabled by this source packet. The approved
native route sheet must identify whether the actual existing command sink or a
separately admitted bounded one-shot delivery consumes the event. Never send
both or manually redeliver while a scheduled consumer can race it.

## 3. Name the refusing process and preserve its result

The receiver is the pinned `github_work.py --config C deliver`, with the
actual frozen Chaski event on stdin. At the separately admitted native step,
the administrator invokes it once through the job-local bound:

```sh
python3 scripts/e579-bounded-delivery.py --job-config "$PRIVATE_JOB_CONFIG"
```

Record the wrapper/source/config hashes, reserved attempt1/max2 and actual
receiver child exit75. Do not also invoke the receiver directly or schedule an
emitter for this private synthetic job. Its configured `StSpawner` invokes:

```text
SHANTY_AGENT=ADMINISTRATOR st [configured prefix/root] agent spawn --ephemeral --job task --item X --repo OWNER/REPO
```

It files X with Seeds before spawning. It runs under the **receiver's actual
Unix UID and administrator identity** from the route sheet. It is not the
still-disabled `_stctl` launchd daemon and not an improvised `st tend` run.
The receiver must run where that administrator's st root and Seeds are
configured; a Linux Chaski service cannot prove a native Mac refusal merely
by naming the Mac's administrator.

With the admitted Mac root showing `unattended=true` and
`unattended_uid_ready=false`, pinned ST `dd11e1ce` refuses with:

```text
unattended ephemeral launches are gated: ephemeral.unattended_uid_ready is false; a distinct protected job UID must be established first
```

Retain actual child exit1, receiver exit75, and the exact detail. A cap,
governor, missing executable, authentication failure, stub success or timeout
is a different outcome and does not count. The receiver must retain one
`requests` row: the exact request ID, `kind=install`, same repo, `work_item=X`,
`state=refused`, `attempts=1`, `claimed_until IS NULL`. Verify no spawned job,
UID-use receipt, metadata or sudo policy was created. Do not rerun to get
cleaner output; an ambiguous result requires a readback first.

## 4. Snapshot on the actual receiver host, without editing the ledger

After the one refused attempt and after the consumer releases its claim, the
ledger's owner takes the snapshot. Read the configured live path rather than
assuming `~/.local/state/chaski/github-work.db`. Use SQLite's online backup API,
not `cp` of a live WAL database. The following is a **later admitted operator
command**, with absolute reviewed live/snapshot paths. It opens the live source
read-only, creates a new private destination, and makes no ledger mutation:

```python
import os, sqlite3
from pathlib import Path
live = Path(LIVE_LEDGER).resolve(strict=True)
snapshot = Path(NEW_PRIVATE_SNAPSHOT)
fd = os.open(snapshot, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600)
os.close(fd)
source = sqlite3.connect(live.as_uri() + '?mode=ro', uri=True)
destination = sqlite3.connect(snapshot)
try:
    source.backup(destination)
finally:
    destination.close()
    source.close()
```

Check snapshot integrity with a read-only `PRAGMA integrity_check` (exactly
`ok`), verify the exact row above using a parameterized request ID, and refuse
any `-wal`, `-shm` or `-journal` companion. Hash the completed standalone
snapshot. Retain source path/host/UID, SQLite/source versions, start/end UTC,
request ID, config and source hashes, child exit and snapshot hash. Export
`sd show X --json` through that deployment's configured board, save mode0600,
verify the exact item ID, and hash it separately. No local competing tracker.

Only then feed these pinned **native** evidence files into the existing
`macos-job-manifest-migration.py preview` command (see the controller packet).
`--max-attempts 2` must match the admitted immutable job config and retained
attempt1/max2 reservation. Verify the receiver ledger independently: a reserved
attempt alone does not prove its child ran or recorded a refusal. A remaining-budget failure stops 1b.
After admitted 1b/metadata/policy/ownkeys/identity qualification, the same
request/item is retried through the admitted normal delivery owner; no manual
ledger edits, lost refusal, duplicate item or UID reuse. Native success and
readiness remain separate later gates.

Rollback/stop: retain the event, spools, item, ledger/snapshot and all refusal
receipts; leave readiness FALSE and the controller disabled. Suspend only the
separately admitted synthetic delivery through its owner's normal lane if it
would retry prematurely. No database row deletion, counter reset, key copying,
identity removal or automatic policy/account rollback is authorized here.
