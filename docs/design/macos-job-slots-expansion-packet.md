# macOS job-slot expansion (e579): design findings, not for admission

Status: **DRAFT for review. Nothing in this packet has been applied.** It was
prepared on the MacBook Air (macOS 15.7.3, arm64) on 2026-11-11 using read-only
commands and `tambo` review mode (no `--apply`, no sudo). `unattended_uid_ready`
stays **FALSE** before, during and after this packet.

Source base: Tambo `48ac88ea4fd8` (v0.13.4). Line references `file:line` are to
that commit, and to Shantytown `371cb4e2` (v0.15.1) where stated.

Read section 2 first. Three findings change what this packet can safely do.

## 1. Scope

In scope, executable after admission:

- Before-state checks of the existing 11-05 installation.
- **Stage S**: create 64 disabled, hidden job accounts, UIDs 60000-60063, with
  private homes and durable non-reuse receipts (`tambo job-slots --apply`).
- **Stage R** (optional, needs a ruling, see 2.1): reviewed removal of installed
  runtime `33de07c8` and install of candidate runtime `c8e9ef3f`.

Written out but **HELD** (cannot succeed or is unsafe as the source stands, see
2.2 and 2.3): job metadata, sudo policy, own-key enrollment, identity policy.

Not in scope: phase-A refusal (aegis-9pu / c01c.5), step 1b launch-field
migration, arming the controller daemon, any change to `unattended_uid_ready`,
any Linux host, any credential or key material.

Owner decisions taken as given (aegis-e579 c93, c96): never-reused per-job UID
slots; 64 slots on this Mac at UIDs 60000-60063; controller stays `_stctl`
51000; crew runs the reviewed pinned sudo packet and Steve watches the first
run; runtime re-pin by reviewed removal with archive, usage proof, post-admission
execution and restore-based rollback.

## 2. Findings that need a ruling before admission

### 2.1 The installed runtime already contains Shantytown e7fcee4

Measured: `git merge-base --is-ancestor e7fcee4 dd11e1ce` succeeds in the
Shantytown checkout. The installed runtime was built from `dd11e1ce`. The
installed `job_launch.py`, `job_metadata.py`, `ephemeral.py`,
`actor_identity.py` and `launcher` are byte-identical to `origin/main`
(`371cb4e2`). Rebuilding from the old pins reproduces manifest `33de07c8…`
and archive `ed2350fb…` exactly.

So the stated reason for the re-pin ("include e7fcee4") is already satisfied.
A re-pin to v0.15.1 changes 63 and adds 33 application files under
`runtime/app/shantytown/`; launcher, interpreter, standard library and `sd` are
unchanged. **Question: is Stage R still wanted, and for which commit?** If not,
drop Stage R entirely; nothing else in this packet depends on it.

### 2.2 The launcher refuses the whole inventory while any slot is provision-only

The privileged helper validates **every** manifest on **every** call
(Shantytown `job_launch.py:311`, `inventory()` at `:127-176`). `load()` requires
all sixteen fields, including the eight launch fields and a frozen `request`
(`:72-74`, `:84-88`). The 64 new manifests are identity-only, like the installed
`eph-task-000001.json`.

Simulated unprivileged with the real launcher code from both runtimes
(ownership checks stubbed, scratch copy of the draft catalog):

```
A. 65 identity-only manifests              -> REFUSED: invalid job manifest schema
B. only eph-task-000001 has launch fields  -> REFUSED: invalid job manifest schema
C. control: all 65 carry launch fields     -> ACCEPTED 65 slots
```

Consequences:

- `job-metadata --apply` calls `st-job-launcher prepare` per slot
  (`src/job_slots.rs:295-321`) and will fail for all 65 slots until every
  manifest carries launch fields.
- Case B: once step 1b gives `eph-task-000001` its launch fields, adding 64
  identity-only slots would make that one launchable slot refuse too.
- `select()` matches a slot on the exact request (`job_launch.py:183`). A slot
  is a capability for one specific job, frozen by a root operation. 64 accounts
  can be pre-created, but 64 launch capabilities cannot be pre-provisioned.

**Question: how do the 64 slots get launch fields?** The source has one
migration script, hard-wired to `eph-task-000001` and to the old catalog pin
(`scripts/macos-job-manifest-migration.py:20-22`).

### 2.3 Ordering conflict with phase A / step 1b, and create-only policy

- `macos-job-manifest-migration.py` refuses unless the installed `slots.json`
  is exactly `21d62832…` (`:22`, `:246-247`). Stage S replaces `slots.json`, so
  running Stage S first makes the reviewed 1b path refuse. Running 1b first
  changes the `eph-task-000001` digest, so this draft catalog must be
  re-rendered, and then case B above applies.
- `job-policy --apply` is create-only for one fragment `st-job-slots`, pinned to
  each manifest digest; a different existing fragment refuses
  (`src/job_slots.rs:333-340`). Applying policy now would pin 65 identity-only
  digests that must all change later, with no supported replacement.

**Question: Stage S before or after phase A + 1b?** Recommendation: Stage S may
run alone now only if the owner accepts that step 1b then needs a new reviewed
migration; otherwise hold Stage S until 1b has run and re-render the catalog.

### 2.4 Smaller points

- The account packet script cannot do the expansion: its installer is
  create-only and refuses a changed `slots.json`
  (`scripts/macos-job-account-packet.py:55-62`, `:186-187`). Stage S therefore
  uses explicit commands. A tested expansion script would fit the repository
  convention better; this is a source gap.
- `bin/tambo-e579` is the `v0.8.0` tag build, which predates the macOS 15
  `dsAttrTypeNative:IsHidden` fix (#222, first tagged in v0.10.0). **Never use
  it for `--apply`.** Stage S uses `bin/tambo-controller-e579` (0.10.1), whose
  `src/job_slots.rs` and `src/job_runtime.rs` are identical to v0.13.4.
- A failure part-way through 64 accounts leaves `prepared` receipts for every
  account not yet completed, and a rerun refuses on them
  (`src/job_slots.rs:535-538`). There is no resume tool.
- All 64 draft principals are kind `task`. The kind is part of the principal,
  the home path and the receipt, and must equal `request.job`
  (`job_launch.py:87`). **Question: what kind split is wanted?**
- `scripts/render-macos-principal-seeds.py:13-16` supports only `_stctl` and
  `_stjobs`. Own-key enrollment for the new slots needs a renderer change.

### 2.5 Rulings recorded 2026-11-11 (aegis-e579)

The findings above were re-checked against Shantytown main `371cb4e2`
(`shantytown/job_launch.py`: `FIELDS`, `load`, `inventory`, `select`) and this
repository (`scripts/macos-job-manifest-migration.py`, `src/job_slots.rs`). The
peer administrator then ruled:

1. Drop the runtime re-pin and the reviewed removal. Stage R below stays as a
   record only and is not to be run; re-pin when a job-path file changes.
2. A source change for multi-slot launch comes first. No 64 accounts are
   created until it lands, so Stage S is not to be run either.
3. Step 1b / phase A runs before any expansion; the catalog is regenerated after.
4. Keep `eph-task-000001` (UID 51001) as a slot.
5. No `job-policy` apply until a replace path exists.
6. Partial-apply recovery for account creation needs a tool and a test.

This document is therefore a design record of the findings. It is not a packet
for admission, and `unattended_uid_ready` stays false.

## 3. Pins

| Input | SHA-256 | Evidence |
| --- | --- | --- |
| `tambo` 0.13.4 (`~/.local/bin/tambo`, review mode only; user-owned, cannot apply) | `f95cabbed30d7d092e19e4f92466903685ead36abb39f7a0b0ac0065011f87c4` | equals `tambo` member of published `tambo-v0.13.4-aarch64-apple-darwin.tar.gz` (`4260a378…8ec0ce`) |
| `bin/tambo-controller-e579` 0.10.1 (root-owned, used for apply) | `35c646c9090cfbf29679ac8bc2754754f49454b1213a98ed1fd37d5c0446436a` | measured; equals the pin in the 11-05 controller packet |
| `bin/tambo-e579` 0.8.0 (preview and `verify` only) | `31c794509649f6065724917bae7014dfb3d64a5c64ada8427bbe480aa12cfba9` | measured |
| Installed `jobs/slots.json` and `controllers/catalog/slots.json` | `21d6283211c709dd3370bf558b5c2e69529a59849b63d48d5b1af1b6c7b5c10a` | measured |
| `eph-task-000001.json` (installed and carried unchanged) | `e985f7f8850b3c3cc6aaaba71865d426e9472a1737ac8c1a8be945e99e990890` | measured, `cmp` equal |
| Draft 65-slot `mac-slots/catalog/slots.json` | `cc5aafc7ce063479e957013470de637f0c4e6c39109636eedb5152c35bb3235d` | rendered; transitively pins 65 manifests |
| `tambo job-slots` review output for the draft | `4500a3b037c6079519d7d1d756fed53e0d5fac444c9a8931c57800fec420fa5d` | identical from 0.13.4, 0.10.1 and 0.8.0 |
| 64-slot variant without `eph-task-000001` (not committed) | `9ed9fd9733b889539f5a26fe5e1041738f7b7fdc2876f6997895598347ed2de1` | rendered with the same script, no `--retain` |
| Installed runtime manifest | `33de07c81561a45b3fd49a8f88160b9761300732cc99c16b21670d5c026acb9a` | measured at `/opt/shantytown-job-launcher/.artifact.json` and the staged artifact |
| Launcher (installed helper, old and candidate artifact) | `03f4ab60513c17677aaef7243efef69396f9210b12be0abd5468566e001aa516` | measured, all three equal |
| Candidate Shantytown commit | `371cb4e20a564ee4207f3097bda31066eeca9f33` (v0.15.1, contains e7fcee4) | `git merge-base --is-ancestor` |
| Candidate Shantytown commit archive (codeload) | `8b415b9ecd43f9080943e3dcc6986cb87e9e4b20cf0c5bc1ef1c916cfd2116a8` | downloaded once on 2026-11-11 |
| Candidate runtime manifest | `c8e9ef3ff02162781a3f172c7ea443ee13a60bbca60fb4bf0ea060cca3d3279d` | built unprivileged in a scratch directory; 1213 files, 88581465 bytes |
| Candidate runtime archive (local build) | `921d2520d1ea3f0944985a119950913e63050233b6b9d6e3c86eb337799b59f0` | local build only; not a CI or release artifact |
| Public CA `pki/lan-svc-ca.pem` | `fb4726c99a24b81edc388a96a8c1f5ef3761472f4b9887b105c04eb61a18fdc8` | measured |

Python and Seeds inputs are unchanged from
`docs/operator-packets/e579/runtime/sources.json`; both downloads matched their
pins. Candidate inputs are in
`docs/operator-packets/e579/mac-slots/runtime/sources.json`.

The draft inventory is `docs/operator-packets/e579/mac-slots/catalog/`:
`eph-task-000001` (UID 51001, `_stjobs`, unchanged bytes) plus `eph-task-00ea60`
to `eph-task-00ea9f` (UIDs 60000-60063, usernames `_stjobs_00ea60` to
`_stjobs_00ea9f`; the suffix is the UID in hex). `uid_min` 51001, `uid_max`
60063, controller UID 51000 throughout. Re-render with:

```sh
python3 scripts/render-macos-job-slot-inventory.py --first-uid 60000 --count 64 \
  --controller-uid 51000 --kind task \
  --retain docs/operator-packets/e579/catalog/eph-task-000001.json --output NEW-DIRECTORY
```

Guards checked against the draft: UID 51001 and 51000 are both at least 10000,
so the existing slot violates no guard and expansion that keeps it is accepted
by `load()` (`src/job_slots.rs:90-168`). On apply, a slot with a complete
matching receipt is read back and kept; slots without a receipt are created
(`src/job_slots.rs:530-558`). Dropping `eph-task-000001` instead would require
deleting its manifest from the protected directory; this packet does not do so.

## 4. Before-state checks

Shell variables used below:

```sh
S='/Library/Application Support/Shantytown'
OLD=33de07c81561a45b3fd49a8f88160b9761300732cc99c16b21670d5c026acb9a
NEW=c8e9ef3ff02162781a3f172c7ea443ee13a60bbca60fb4bf0ea060cca3d3279d
CAT=docs/operator-packets/e579/mac-slots/catalog
```

Unprivileged. Each was run on 2026-11-11 with the result shown.

```sh
shasum -a 256 "$S/jobs/slots.json" "$S/jobs/eph-task-000001.json" \
  "$S/bin/tambo-controller-e579" /usr/local/libexec/st-job-launcher \
  /opt/shantytown-job-launcher/.artifact.json          # pins in section 3
tambo job-slots --directory "$S/jobs"                  # exit 0, one account, UID 51001
tambo job-runtime --directory "$S/runtime-artifacts/$OLD" --manifest-sha256 $OLD
                                                       # exit 0, files 1180, bytes 88163686
tambo job-slots --directory "$CAT" | shasum -a 256     # 4500a3b0…20fa5d
dscl . -list /Users UniqueID | awk '$2>=50000 && $2<70000'   # only _stctl 51000, _stjobs 51001
dscl . -list /Groups PrimaryGroupID | awk '$2>=50000 && $2<70000'   # same two
ls /private/var/lib/shantytown/agents                  # only crew-eph-task-000001
ps -axo user,uid,pid,command | awk '$2==51000||$2==51001||($2>=60000&&$2<=60063)'   # empty
ps -axo user,pid,command | grep -E 'shantytown-job-launcher|st-job-launcher' | grep -v grep   # empty
launchctl list | grep -i -E 'shanty|stjob|stctl|tambo'  # only user agent shantytown.st-tend
ls /Library/LaunchDaemons | grep -i shanty             # only local.shantytown.pf.plist
ls /private/etc/sudoers.d                              # expect no st-job-slots fragment
ls "$S/jobs/used" /private/var/lib/shantytown/job-stores "$S/identity.json"   # all absent
```

Neither launchd entry references the runtime: `shantytown.st-tend` runs the
user's pipx `st`, and `local.shantytown.pf` runs `pfctl`.

Privileged read-only checks, to run at admission (not run for this draft):

```sh
sudo /bin/cat "$S/jobs/account-receipts/51001.json"
# expect {"principal":"eph-task-000001","sha256":"e985f7f8…990890","state":"complete","uid":51001}
sudo /usr/bin/dscl . -read /Users/_stjobs AuthenticationAuthority     # expect ;DisabledUser;
sudo /usr/bin/python3 -I scripts/macos-job-account-packet.py verify
# expect: VERIFIED disabled-login accounts, …   and   rollback tombstone: False
sudo /usr/sbin/lsof +D /opt/shantytown-job-launcher                   # expect no output
sudo /usr/sbin/visudo -c                                              # expect parsed OK
sudo /bin/launchctl print system | grep -i -E 'shantytown|stjob|stctl'   # expect only local.shantytown.pf
```

Stop if any result differs.

## 5. Stage S — expand to 64 new slots (accounts only)

Preconditions: sections 2.2-2.3 ruled on; section 4 clean; this commit and the
catalog pin `cc5aafc7…` reviewed. `cd` to the reviewed checkout.

S1. Archive the current plan (create-only, root-owned):

```sh
sudo /bin/mkdir -m 755 "$S/jobs/migrations" "$S/jobs/migrations/archive"
sudo /bin/cp -n "$S/jobs/slots.json" "$S/jobs/migrations/archive/slots.21d62832.json"
sudo /bin/chmod 444 "$S/jobs/migrations/archive/slots.21d62832.json"
shasum -a 256 "$S/jobs/migrations/archive/slots.21d62832.json"   # expect 21d62832…
```

S2. Check pins, then install the 64 new manifests create-only:

```sh
shasum -a 256 "$CAT/slots.json"                        # expect cc5aafc7…
tambo job-slots --directory "$CAT" | shasum -a 256     # expect 4500a3b0…
cmp "$CAT/eph-task-000001.json" "$S/jobs/eph-task-000001.json"   # expect silence
for f in "$CAT"/eph-task-00ea??.json; do
  sudo /bin/cp -n "$f" "$S/jobs/" || exit 1
done
sudo /bin/chmod 644 "$S"/jobs/eph-task-00ea??.json
ls "$S"/jobs/eph-task-00ea??.json | wc -l              # expect 64
```

From here until S3 completes every tool refuses with "manifest set differs
from reviewed slots". That is the intended fail-closed state.

S3. Replace the plan atomically:

```sh
sudo /bin/cp "$CAT/slots.json" "$S/jobs/.slots.json.new"
sudo /bin/chmod 644 "$S/jobs/.slots.json.new"
sudo /bin/mv -f "$S/jobs/.slots.json.new" "$S/jobs/slots.json"
shasum -a 256 "$S/jobs/slots.json"                     # expect cc5aafc7…
"$S/bin/tambo-controller-e579" job-slots --directory "$S/jobs" | shasum -a 256   # expect 4500a3b0…
```

S4. Create the accounts with the root-owned 0.10.1 tool. **Not `tambo-e579`.**

```sh
sudo "$S/bin/tambo-controller-e579" job-slots --directory "$S/jobs" --apply | tee S4.out
shasum -a 256 S4.out                                   # expect 4500a3b0… and exit 0
```

Expected behaviour (`src/job_slots.rs:499-617`): 51001 is read back against its
complete receipt; 64 `prepared` receipts are written first; each account then
gets its own group, disabled authentication, `/usr/bin/false`, hidden record and
a 0700 home, is read back, and its receipt becomes `complete`. Duration is not
measured; it runs about fourteen native commands per account.

S5. Verify:

```sh
sudo /usr/bin/python3 -I scripts/macos-job-account-packet.py verify
# expect: VERIFIED disabled-login accounts, unique UIDs/GIDs, private homes and durable receipts
dscl . -list /Users UniqueID | awk '$2>=60000 && $2<=60063' | wc -l   # expect 64
sudo /bin/ls "$S/jobs/account-receipts" | wc -l        # expect 65
sudo /usr/bin/grep -L '"state":"complete"' "$S"/jobs/account-receipts/*.json   # expect no output
for u in _stjobs_00ea60 _stjobs_00ea9f; do dseditgroup -o checkmember -m $u admin; done
# expect "no <name> is NOT a member of admin" twice
```

Refusals to expect instead of success, and what they mean:

| Message | Meaning | Action |
| --- | --- | --- |
| `slot account/UID/home is already occupied; no adoption` | a name, UID, GID or home in 60000-60063 exists | stop; nothing was created |
| `retained UID reservation needs explicit recovery; never recycle it` | an earlier run stopped part-way | stop; keep receipts; ask for a recovery ruling |
| `retained job identity differs from native accounts` | 51001 drifted from its receipt | stop |
| `job account is only partially provisioned` | read-back failed after creation | stop; keep receipts |
| `account plan path is not immutable root-owned input` | wrong binary or a writable path | stop |

Rollback of Stage S. Accounts, homes and receipts are never deleted and UIDs
are never reused. Restore only the previous inventory view:

```sh
sudo /bin/mkdir -m 755 "$S/jobs/migrations/withdrawn-cc5aafc7"
sudo /bin/mv "$S"/jobs/eph-task-00ea??.json "$S/jobs/migrations/withdrawn-cc5aafc7/"
sudo /bin/cp "$S/jobs/migrations/archive/slots.21d62832.json" "$S/jobs/.slots.json.new"
sudo /bin/chmod 644 "$S/jobs/.slots.json.new"
sudo /bin/mv -f "$S/jobs/.slots.json.new" "$S/jobs/slots.json"
tambo job-slots --directory "$S/jobs"                  # expect one account, UID 51001
```

After a partial S4, do this only on an explicit ruling; the default is to stop
and preserve state. The existing tombstone rollback
(`scripts/macos-job-account-rollback.sh`) remains available and blocks any
later run of the original setup script.

## 6. Stage R — runtime re-pin by reviewed removal (optional, see 2.1)

Why removal is needed: `tambo job-runtime --apply` is "never an upgrade"
(`src/job_runtime.rs:306`). With a root present it only reads back the same
artifact (`:342-343`); a different one fails in `verify_installed` with
`installed runtime inventory changed` (`:288-289`) or `installed runtime
differs; explicit recovery required` (`:299-300`). A different helper fails with
`different helper installed; explicit recovery required` (`:334-338`). The
helper bytes are the same in both artifacts, so the helper stays in place.

Run only when no job is active and after admission.

R0. Build inputs, unprivileged, already done once in a scratch directory:

```sh
curl -fsSL -o INPUTS/shantytown.tar.gz \
  https://codeload.github.com/intigraph/shantytown/tar.gz/371cb4e20a564ee4207f3097bda31066eeca9f33
# python.tar.gz and seeds.tar.gz: URLs in mac-slots/runtime/sources.json
shasum -a 256 INPUTS/*.tar.gz        # must equal the three pins in that file
python3 -c 'import importlib.util,sys
from pathlib import Path
s=importlib.util.spec_from_file_location("b","scripts/build-macos-job-runtime.py")
b=importlib.util.module_from_spec(s); s.loader.exec_module(b)
i,o=Path(sys.argv[1]),Path(sys.argv[2])
print(b.build(i/"python.tar.gz",i/"shantytown.tar.gz",i/"seeds.tar.gz",o,
      sources=Path("docs/operator-packets/e579/mac-slots/runtime/sources.json")))
t=o.with_name(o.name+".tar.gz"); b.pack(o,t); print(b.digest(t))' INPUTS OUT/macos-arm64-job-runtime
# expect c8e9ef3f…3279d then 921d2520…b59f0
tambo job-runtime --directory OUT/macos-arm64-job-runtime --manifest-sha256 $NEW
# expect exit 0, "files": 1213, "bytes": 88581465
```

The build script has no flag for an alternative sources file, hence the
`python3 -c` form. The fetch script reads only the old pinned file.

R1. Archive the installed runtime (ruling condition a):

```sh
sudo /bin/mkdir -m 755 "$S/runtime-archive"
sudo /usr/bin/tar -czf "$S/runtime-archive/job-runtime-$OLD.tar.gz" \
  -C /opt shantytown-job-launcher -C /usr/local/libexec st-job-launcher
sudo /bin/chmod 444 "$S/runtime-archive/job-runtime-$OLD.tar.gz"
shasum -a 256 "$S/runtime-archive/job-runtime-$OLD.tar.gz"   # RECORD on aegis-e579; not predictable
sudo "$S/bin/tambo-controller-e579" job-runtime --directory "$S/runtime-artifacts/$OLD" \
  --manifest-sha256 $OLD --apply                       # expect "changed": false
```

The last command is the installer's exact read-back of the installed tree. The
staged artifact `runtime-artifacts/33de07c8…` is a second complete copy and is
left in place.

R2. Show nothing uses it (ruling condition b). Repeat the `ps`, `launchctl`,
`lsof +D`, `sudoers.d`, `jobs/used` and `job-stores` checks from section 4
immediately before R3. Measured on 2026-11-11: no process, no launchd entry, no
sudo policy, no consumed-slot receipt and no job store refers to it. `lsof` as
root was not run.

R3. Remove by renaming aside (atomic and reversible; no recursive delete):

```sh
sudo /bin/mv /opt/shantytown-job-launcher /opt/shantytown-job-launcher.removed-33de07c8
/usr/local/libexec/st-job-launcher check eph-task-000001 $OLD   # expect failure: interpreter missing
```

R4. Stage and install the candidate:

```sh
sudo /usr/bin/python3 -I scripts/macos-job-runtime-stage.py --archive "$ARCHIVE" \
  --archive-sha256 921d2520d1ea3f0944985a119950913e63050233b6b9d6e3c86eb337799b59f0 \
  --manifest-sha256 $NEW
# expect: STAGED protected artifact …/runtime-artifacts/c8e9ef3f…; no runtime apply or readiness grant
sudo "$S/bin/tambo-controller-e579" job-runtime --directory "$S/runtime-artifacts/$NEW" \
  --manifest-sha256 $NEW --apply
# expect "changed": true, "files": 1213, "bytes": 88581465
shasum -a 256 /opt/shantytown-job-launcher/.artifact.json /usr/local/libexec/st-job-launcher
# expect c8e9ef3f… and 03f4ab60…
```

`$ARCHIVE` is an absolute path to the reviewed tar. If the reviewed archive is
a CI or release build, replace the archive pin with that build's checksum; the
manifest pin is the content boundary and must not change.

Rollback of Stage R (ruling condition d):

```sh
sudo /bin/mv /opt/shantytown-job-launcher /opt/shantytown-job-launcher.removed-c8e9ef3f
sudo /bin/mv /opt/shantytown-job-launcher.removed-33de07c8 /opt/shantytown-job-launcher
sudo "$S/bin/tambo-controller-e579" job-runtime --directory "$S/runtime-artifacts/$OLD" \
  --manifest-sha256 $OLD --apply                       # expect "changed": false
```

If the renamed tree is unavailable, unpack the R1 archive after checking its
recorded checksum, then run the same read-back. Previous launchd state is "no
runtime-related job loaded"; confirm with the section 4 `launchctl` checks. Do
not roll back while a job is running; no job can be running before policy
exists.

## 7. HELD steps: metadata, policy, own keys, identity

Each needs the rulings in section 2 and, for own keys, a source change. The
commands are recorded so the reviewer can see exactly what is being held.

Metadata (after every manifest carries launch fields; runtime must be installed):

```sh
sudo "$S/bin/tambo-controller-e579" job-metadata --directory "$S/jobs" --apply
# success: {"changed":true}; helper answers PREPARED <digest> CREATED per slot
# with the draft catalog: Error: job metadata preparation unproved; retain its state
```

Policy (after metadata success; create-only, so apply once with final digests):

```sh
tambo job-policy --directory "$S/jobs" | shasum -a 256   # review the exact fragment first
sudo "$S/bin/tambo-controller-e579" job-policy --directory "$S/jobs" --apply   # {"changed":true}
sudo /usr/sbin/visudo -c
sudo /usr/bin/sudo -l -U _stctl
sudo /usr/bin/sudo -l -U _stjobs_00ea60
```

For the draft catalog the rendered fragment is 132 lines, 53031 bytes, SHA-256
`81ad79ec926bfc53f12a8b046bd48ea78426e747e3a60fe80f981ab68773019c`, and
`visudo -cf` parses it. That digest is informational: it pins identity-only
manifests and must not be installed. Rollback of policy: move the exact
`st-job-slots` fragment to a protected archive outside `sudoers.d`, run
`visudo -c`, confirm `sudo -l -U` shows no helper rule.

Own-key enrollment: per principal, a reviewed one-shot launchd job with
`UserName`/`GroupName` of that account runs
`/opt/shantytown-job-launcher/bin/sd key init --session P --agent P --introducer I`
in its own home; then a root-owned `seeds-config/<username>` directory with the
CA pin and that UID's own signing fields; then the identity policy row. See
`docs/operator-packets/e579/principal-seeds.md`. The renderer rejects every
username other than `_stctl` and `_stjobs`. Rollback: boot out and disable only
the one-shot label; keep keys, logs and config.

## 8. Negative controls after install (recipe only; none has been run)

Run with synthetic credentials and a synthetic request. Any cancelled, missing
or ambiguous result is UNKNOWN, not PASS. Record command, UTC time, exit status
and sanitized output for each.

Identity cannot be borrowed:

1. Inside a launched job, delete then spoof `SHANTY_AGENT`, `SHANTY_ROOT`,
   `HOME`, `USER`; `st` must still resolve the job's own principal or refuse.
2. Inside a job, `setsid` a child and repeat step 1; `id -u` stays the slot UID.
3. Inside a job, `cd` to `/`, to the admin user's home and to another slot's home; the
   last two must fail, and identity must not change.
4. From a job: read the admin user's home, `/var/lib/shantytown/controllers/_stctl`, another
   slot's home and key; all denied. `sudo -n true` denied. `sudo -n -l` lists
   only `submit` for its own principal and digest.

Launch must refuse:

5. Wrong UID: call `launch` for slot A as a UID that is not 51000.
6. Unbound UID: slot with no identity policy row.
7. Current-user fallback: run `st agent spawn` unattended as UID 501 with no
   matching capability; it must refuse, never run as 501.
8. Failed switch: a manifest whose account was made unresolvable in a fixture;
   the slot is consumed and nothing runs as root or controller.
9. Missing user: catalog entry whose account does not exist.
10. Partial provisioning: account present, home mode wrong, or receipt `prepared`.
11. Readiness mismatch: `unattended_uid_ready = false` with an otherwise valid
    capability; expect "unattended ephemeral launches are gated".
12. Wrong manifest digest, and replay of a consumed slot (`job UID already used`).

Capacity and reuse:

13. Exhaustion: with every slot consumed or bound, a new request refuses with
    "job needs exactly one unused protected launch capability".
14. Non-reuse: `jobs/used/<principal>.json` and `account-receipts/<uid>.json`
    persist after stop and after rollback; a second launch on that UID refuses.

Lifecycle is own-job-only:

15. Slot A `submit` with slot B's principal or digest refuses; controller
    `stop`/`poll` for a slot it does not control refuses; `stop` signals only
    processes of that slot's UID.
16. No admin credential is visible to a job: environment, open descriptors,
    files under its home and `/Library/Application Support/Shantytown`.

Regression and rollback:

17. Ordinary attended launch and retry as the human or an administrator behave
    as before.
18. After rollback of policy and runtime, active-job state, all 66 accounts,
    all receipts and all homes are still present and unchanged.

Only after all pass does a separate decision consider `unattended_uid_ready`.

## 9. Open questions and things not verified

Need a ruling:

1. Is the runtime re-pin still wanted now that `33de07c8` is shown to contain
   e7fcee4? If yes, confirm commit `371cb4e2`.
2. How do 64 slots receive launch fields, given one frozen request per slot and
   whole-inventory validation?
3. Does Stage S run before or after phase A and step 1b?
4. Keep `eph-task-000001`/51001 in the catalog (drafted: 65 slots, range
   51001-60063) or drop it (64 slots, range 60000-60063, needs a reviewed
   deletion of its manifest)?
5. Kind split of the 64 principals.
6. Is rename-aside acceptable as "removal", in addition to the tar archive?
7. Plain commands for Stage S, or a tested expansion script first?
8. Which runtime archive is the reviewed input: this local build, or a CI or
   release build after `sources.json` is updated in place?
9. Recovery procedure if S4 stops part-way.

Not verified (no root was used):

- Contents of `jobs/account-receipts/51001.json` and `controllers/51000.json`
  (mode 0600). Their sizes are 138 and 135 bytes; 138 is the exact length of the
  expected complete receipt, which is consistent but not proof.
- `AuthenticationAuthority` of both accounts: an unprivileged `dscl -read`
  returns "No such key".
- `/etc/sudoers` contents; that `sudoers.d` is included.
- Open files under `/opt/shantytown-job-launcher` held by other users' processes.
- Any `--apply` path. Every statement about apply behaviour is from reading the
  source at the cited lines.
- How the first account was completed on 11-03 with a v0.8.0 tool that predates
  the macOS 15 read-back fix.
- Duration and failure rate of creating 64 accounts in one run on macOS 15.
- Whether the published v0.13.4 release asset `macos-arm64-job-runtime.tar.gz`
  (`f9c8dad7…`) carries manifest `33de07c8`; its checksum differs from the local
  reproducible archive `ed2350fb…`.
- The launcher simulation in 2.2 stubs the root-ownership check. Against the
  real installed one-slot inventory, with no stub, the same code also returns
  "invalid job manifest schema".
