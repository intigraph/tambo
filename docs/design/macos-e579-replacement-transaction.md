# Exact controller tool and runtime replacement

This is source for the next admitted operator runtime batch. A green PR,
published release, downloaded archive or successful unprivileged preview
does **not** authorize running it as root. No live account, key, policy,
catalog, launchd job or readiness flag is changed by this source work.

The old create-only account and runtime installers retain their contracts.
Do not substitute a new binary hash into the old controller setup commands.
The finite replacement implementation is
`scripts/macos-e579-runtime-transaction.py`, standard-library Python 3.9+.
Its native CLI accepts only macOS root, fixed production paths and these
predecessor/successor bytes:

| Object | Before | After |
| --- | --- | --- |
| `bin/tambo-controller-e579` | `35c646c9090cfbf29679ac8bc2754754f49454b1213a98ed1fd37d5c0446436a` | `af3730602aad22542e775c71614476792b2cfc8e6626ef7e7561904ee0fc32b3` |
| Runtime manifest | `33de07c81561a45b3fd49a8f88160b9761300732cc99c16b21670d5c026acb9a` | `25cadd5a13a3f8021f9fb11bedfb789e815b5cb10141962f4fdba9ca35fb85f1` |
| Fixed launcher | `03f4ab60513c17677aaef7243efef69396f9210b12be0abd5468566e001aa516` | unchanged |

The qualified Tambo v0.14.1 source/tag is
`0f291ca0839d93db8dc8ee8891d7cda1d7c0111f`; its arm64 tool archive SHA256 is
`85c68cf0c59819c5569ee89e65cde1df0854991c192a38187d794fbfa53dcfe2`.
The actual published runtime archive SHA256 is
`42780e8f25740f79bce6d21ae13d855ba058bae54e1a652ae978cef1939dd040`.
The reviewed manifest, not the retained local tar encoding, binds all 1220
public runtime members. The pinned existing launcher must remain identical.

## Admission and A0

The operator batch first disables **all** job admission and establishes sole
writer custody. Keep it disabled until this transaction and its possible
inverse finish. Advisory locks do not fence an unrelated root administrator;
the admitted freeze is an operator obligation, never inferred from an idle
process sample. Any other writer or uncertain freeze means STOP.

The three Python source files (this transaction, the existing controller
packet module and the existing runtime staging module) must be frozen under
root-owned, non-writable, non-symlink parents at their exact reviewed hashes.
The qualified new executable and tar must likewise be protected inputs, not
user-writable downloads used by root. The packet lists their absolute paths.
No helper downloads, installs dependencies, accepts an alternate production
root, discovers a moving release or creates signing credentials.

Separately admitted A0 runs `python3 -I SOURCE a0`. It emits one JSON receipt
on stdout and makes no transaction/lock/backup file. Operator evidence capture
is external to A0. Retain stderr and the actual exit status; a refusal or
unknown instrument is not a partial PASS. The receipt has `native_ready:false`.
Its complete stdout bytes, including final newline, are hashed into each
mutating phase's admission packet.

A0 requires exact complete root-0600 controller and job account receipts;
the original identity-only job and catalog hashes; both native disabled-login
identities, unique UID/GID and non-admin/group isolation; absent policy,
identity/used/migration/rollback/controller-store/launchd activation state;
whole effective sudo denial for both principals; successful root process and
open-file inventories; and no reader of the tool, fixed helper or runtime.
UIDs 60000–60063 must remain absent. This is a first-slot replacement, not
65-account expansion or phase-A execution. Missing or partial receipts,
unexpected native output, inaccessible lsof coverage, warnings, symlinks,
extra runtime members or a changed byte/mode refuse. Native output is never
copied into the public receipt. A0's root probes require their own admission;
the private read-only Lex backup is not a root rollback receipt.

## Mutating phase packet

For **each** separately authorized phase, the runtime owner supplies a
root-owned mode-0600 JSON file and its exact SHA256. The exact schema is:

```json
{
  "version": 1,
  "transaction": "e579-0141",
  "phase": "a1",
  "expires_at": "REPLACE_WITH_ADMITTED_UTC_EXPIRY",
  "freeze": "all-job-admission-disabled",
  "a0_sha256": "REPLACE_WITH_COMPLETE_A0_STDOUT_SHA256",
  "sources": {
    "macos-e579-runtime-transaction.py": "REPLACE_WITH_REVIEWED_SHA256",
    "macos-controller-account-packet.py": "REPLACE_WITH_REVIEWED_SHA256",
    "macos-job-runtime-stage.py": "REPLACE_WITH_REVIEWED_SHA256"
  },
  "tool": "REPLACE_WITH_ADMITTED_PROTECTED_ABSOLUTE_EXECUTABLE",
  "archive": "REPLACE_WITH_ADMITTED_PROTECTED_ABSOLUTE_RUNTIME_TAR"
}
```

Placeholders deliberately refuse. The UTC expiry must be in the future and
at most two hours away. Packet possession is not independent authorization:
the root owner of the admitted batch freezes and supplies it, never the job
principal. No receipt claims a brokered/signed authorization protocol.
Each phase rechecks live prerequisites and binds the same A0 snapshot under
the existing controller packet, account, policy and runtime writer locks.
The exact paths are `.controller-packet-lock`, `jobs/.account-lock`,
`/private/etc/sudoers.d/.st-job-slots.lock`, and
`/opt/.shantytown-job-runtime.lock`. A0 does not create these; mutation may
create a missing lock at mode0600 under a verified protected parent.

Invoke only the phase named by its packet, using the reviewed protected
source: `python3 -I SOURCE a1 --admission PACKET --admission-sha256 SHA
--tool TOOL`; A2 adds `--archive ARCHIVE` instead of `--tool`. A3 takes neither
input. These are syntax descriptions, **not** an admitted sudo command block.
The runtime batch must bind actual source/script hashes, paths, expiry,
freeze owner and rollback before replacing the placeholders.

## A1, A2 and A3

A1 starts only with the old tool/runtime and absent transaction destinations.
It creates mode0700
`/Library/Application Support/Shantytown/e579-replacement-0141`, an exclusive
prepared journal, the exact old tool backup, and two protected full old-runtime
copies at `/opt/.e579-0141-runtime-backup` and
`/opt/.e579-0141-runtime-restore-proof`. Every inventory/hash/mode is independently
read back, and files/directories are synced before tool replacement. The tool
is staged at its protected same-parent `.tambo-controller-e579-0141.pending`;
predecessor bytes and prerequisites are checked again before atomic replacement
and exact new-byte readback. The old `tambo-e579` binary is untouched.

A2 verifies the protected published archive using the existing safe archive
parser and the fixed archive/manifest pins. It creates the exact runtime
layout at `/opt/.e579-0141-new-runtime`, including the manifest receipt, and
verifies/syncs every member before touching the old runtime. Under the same
locks and repeated native checks it renames the exact old tree to
`/opt/.e579-0141-old-runtime`, then renames the prepared new tree to the absent
fixed runtime destination. No recursive deletion or live-process interruption
exists. Journals and partial trees survive each error. A1/A2 never resume an
interrupted attempt automatically.

A3 is an inverse **only before any phase-A use, catalog/metadata migration,
policy, identity/key or controller-store activation**. It repeats the live
prerequisites, verifies both protected backup/restore copies, and accepts only
recognized exact old/new tool and runtime states. A new runtime is retained at
`/opt/.e579-0141-retained-new-runtime`; the exact renamed old tree is restored.
The new executable is retained in the transaction directory before the old
tool is atomically restored. No failed output is deleted. An interrupted
rename can be inverted if its exact state is proved; an unknown or modified
tree refuses rather than guessing which copy is current. A completed inverse
has an exclusive rolled-back receipt and cannot replay.

No transaction step enables readiness. After successful replacement the
remaining admitted sequence is still phase-A refusal, exact 1b expansion of
the original manifest, metadata/policy, own keys/identity and independent
native negatives. Once any of those begins, A3 is unavailable: rollback then
requires a new scoped disposition preserving live state and UID nonreuse.

## Qualification

`python3 scripts/test-macos-e579-runtime-transaction.py` uses only disposable
files and mock native commands. It covers forward/inverse byte preservation,
both protected copies, changed A0 binding, competing writer locks, account
receipt state, symlinks, extra/modified members, changed prerequisites after
use, unknown native probes and failure between the two runtime renames.
The Python 3.9/3.12 offline CI matrix runs these controls. They establish no
native Mac execution, protected root backup, privileged receipt completion,
freeze custody, account enrollment or isolation readiness.
