# macOS controller and immutable runtime packet

This is source preparation for a human operator. It creates no live account,
key, sudo grant or readiness flag in CI. Use one reviewed commit and attach its
script hashes, the tool/archive hashes below, and the complete private input
packet to the operator request. Do not select a moving release as root.

The existing job-account packet remains separate. Its first UID is 51001;
this packet creates only `_stctl`, UID/GID 51000, hidden, disabled-login,
`/usr/bin/false`, and private home `/var/lib/shantytown/controllers/_stctl`.
The account input is `docs/operator-packets/e579/controller.json`, SHA256
`aff7d04c3b67f6cb068c6ee92c3f02012ed724a7d55b6138fc68036c6874dd9c`.
The catalog pin is
`21d6283211c709dd3370bf558b5c2e69529a59849b63d48d5b1af1b6c7b5c10a`.
The controller UID must equal the catalog member's `controller_uid`; UID 501
and the job UID are refused. Accounts, UID/GID aliases and occupied homes are
never adopted. A prepared reservation survives any native failure and refuses
automatic recovery; completed exact reruns verify without native mutation.

Account scripts support the native Python 3.9 standard library. The runtime
bundles its own **arm64 Python 3.12.16**, independently of `/usr/bin/python3`.

## Public artifact and immutable inputs

`docs/operator-packets/e579/runtime/sources.json` pins every input:

- Python standalone build 20261026, native arm64 CPython 3.12.16, archive
  `fbf4afef53c2a6b72bb741bc06d5671a9dfaf87bd84e5aa73ac9b3d0d589f14e`.
- Shantytown commit `beb5d2180a19a69b6b5a715ab4be2ab06efbdd27`
  (published v0.16.0), commit archive
  `a6532e60e7c78f4eca6777d17e0fb45387df3745387918b698786c547f5003f3`.
  The source distribution omits `scripts/st-job-launcher`, so the builder uses
  this checksum-pinned complete commit archive instead.
- Seeds 0.14.0 native arm64 archive
  `74a996635b7ece44ef896581b0689ed85d1a7b7c307334927a19a1a04c1e779a`.

The artifact manifest pin is
`25cadd5a13a3f8021f9fb11bedfb789e815b5cb10141962f4fdba9ca35fb85f1`.
It binds 1220 regular files, about 88 MB: interpreter, standard library and
native libraries, public Shantytown application, launcher, trusted `sd`, and
licenses. Archive links are materialized from the pinned archive; output has
no symlinks, site packages, `.pth` hooks, tests, bytecode, private settings,
provider logins, CA material or keys. The runtime is not a venv.

Build without root from downloaded, pinned local inputs:

```sh
python3 scripts/fetch-macos-job-runtime-inputs.py --directory INPUTS
python3 scripts/build-macos-job-runtime.py \
  --python-archive INPUTS/python.tar.gz \
  --shantytown-archive INPUTS/shantytown.tar.gz \
  --seeds-archive INPUTS/seeds.tar.gz --output macos-arm64-job-runtime
python3 scripts/verify-macos-job-runtime.py --directory macos-arm64-job-runtime
```

On an arm64 macOS CI runner add `--native` to prove private-prefix isolated
imports without root. CI publishes the tar, its `.sha256` sibling and manifest
as the immutable `macos-arm64-job-runtime` Actions artifact. The normal Tambo
release job builds and attaches the same named tar/checksum to its release;
there is no manual tag or release-asset mutation in this packet. Record the
actual release URL, tar SHA256 and exact CI source commit before asking the
operator to use it. `runtime/artifact.json` records the retained local build;
the separately reviewed manifest pin is the content boundary.

The separately reviewed Tambo input is the public
`tambo-v0.10.1-aarch64-apple-darwin.tar.gz`, archive SHA256
`9f5f808094dd4a5afff204186161e4c65c7a13dc25482b5bf1404bf73f4aa324`.
The `tambo` executable SHA256 is
`35c646c9090cfbf29679ac8bc2754754f49454b1213a98ed1fd37d5c0446436a`.
Setup installs it create-only at
`/Library/Application Support/Shantytown/bin/tambo-controller-e579`.
It leaves the earlier `tambo-e579` copy alone. A different occupied tool
refuses; replacement requires its own explicit reviewed pins and packet.

## Operator sequence and exit receipts

The owner-acknowledged order is controller -> runtime -> phase-A refusal ->
explicit 1b expansion -> metadata -> policy -> ownkeys -> identity. The
[phase-A runbook](../operator-packets/e579/phase-a.md) records the exact
producer/receiver/snapshot procedure and the still-held route/retry contract.
The [own-UID key/config packet](../operator-packets/e579/principal-seeds.md)
renders disabled launchd jobs and complete protected Seeds config candidates.
Neither source packet grants native execution or readiness.

Run previews unprivileged first, from the exact reviewed source. The examples
use `TOOL`, `ARCHIVE` and `ARCHIVE_SHA256` as **operator-reviewed absolute input
paths and the actual published checksum**, not discovered shell output. Retain
sanitized stdout/stderr and exit status at every step. A nonzero result stops
that sequence; do not continue with `|| true` or delete a prepared reservation.

0. Check the public tar checksum and manifest pin. On the Mac:

```sh
/usr/bin/python3 -I scripts/macos-controller-account-packet.py verify --offline \
  --packet docs/operator-packets/e579/controller.json \
  --packet-sha256 aff7d04c3b67f6cb068c6ee92c3f02012ed724a7d55b6138fc68036c6874dd9c \
  --inventory docs/operator-packets/e579/catalog \
  --inventory-sha256 21d6283211c709dd3370bf558b5c2e69529a59849b63d48d5b1af1b6c7b5c10a
"$TOOL" job-runtime --directory macos-arm64-job-runtime \
  --manifest-sha256 25cadd5a13a3f8021f9fb11bedfb789e815b5cb10141962f4fdba9ca35fb85f1
```

The offline message explicitly says native controller/readiness is unproved.
Preview does not install anything. Also reject UID collisions, UID below10000,
wrong input pins, symlinks and changed occupied destinations in the disposable
source controls. Source tests do not establish native isolation. An unsupported
Darwin HOME-guard mount inventory is UNKNOWN, never a guarded native PASS.

1. **Controller account** (operator sudo only):

```sh
sudo /usr/bin/python3 -I scripts/macos-controller-account-packet.py setup \
  --tool "$TOOL" \
  --tool-sha256 35c646c9090cfbf29679ac8bc2754754f49454b1213a98ed1fd37d5c0446436a \
  --packet docs/operator-packets/e579/controller.json \
  --packet-sha256 aff7d04c3b67f6cb068c6ee92c3f02012ed724a7d55b6138fc68036c6874dd9c \
  --inventory docs/operator-packets/e579/catalog \
  --inventory-sha256 21d6283211c709dd3370bf558b5c2e69529a59849b63d48d5b1af1b6c7b5c10a
sudo /usr/bin/python3 -I scripts/macos-controller-account-packet.py verify \
  --tool-sha256 35c646c9090cfbf29679ac8bc2754754f49454b1213a98ed1fd37d5c0446436a \
  --packet docs/operator-packets/e579/controller.json \
  --packet-sha256 aff7d04c3b67f6cb068c6ee92c3f02012ed724a7d55b6138fc68036c6874dd9c \
  --inventory docs/operator-packets/e579/catalog \
  --inventory-sha256 21d6283211c709dd3370bf558b5c2e69529a59849b63d48d5b1af1b6c7b5c10a
```

Verify checks the macOS15 `dsAttrTypeNative:IsHidden` spelling, unique UID/GID,
disabled hidden false-shell account, no admin membership, no shared groups with
the UID501 login, private home and complete0600 root-owned nonreuse receipt.
It makes no key, policy or daemon activation claim.

1b. **Explicit expansion of the never-launched slot**, using
`scripts/macos-job-manifest-migration.py`. This is a separate migration, never
an implicit operation of controller setup, metadata or policy. The old eight
identity fields retain their serialized bytes. Supply a private JSON object
containing **only** the eight launch fields, frozen by the operator: supported
harness, exact request, pinned argv, private workspace/settings, separate
controller root, allowlisted environment and null review for this synthetic task.
No harness/model binary or provider login is needed for this lifecycle probe.

The manifest has no `bin/st`. The immutable entrypoint is
`/opt/shantytown-job-launcher/bin/python3`, followed by `-BIS`, `-c`, the fixed
bootstrap `import sys; sys.path.insert(0,"/opt/shantytown-job-launcher/app"); from shantytown.cli import main; raise SystemExit(main())`,
then `agent spawn --report --verdict done --summary` and the reviewed summary
as one final argument. The controller root must be canonical
`/private/var/lib/shantytown/controllers/_stctl/root`: the pinned selector
compares the manifest string against `Path(root).resolve()`. The disabled
controller plist uses that same root. Paths remain private operator inputs.

**Two-phase dependency:** the synthetic event first files item X and refuses
without a launch capability. Only then can `request.item` be frozen to X. Retain
that refusal and the remaining retry budget. Supply a consistent standalone
Chaski SQLite snapshot (no WAL/journal companions), exact synthetic request ID,
actual configured maximum attempts, and a `sd show X --json` item readback.
The script requires SHA256 pins for both files and cross-checks the unique
ledger request: kind `install`, same repo/item, state `refused`, no live claim,
and `1 <= attempts < max_attempts`. Evidence snapshots are operator-attested
inputs; the script never queries a live board or authenticates a snapshot's
origin. Record the live read commands and their provenance with the private
packet. The root helper's protected input cannot be prepared with a guessed X.

```sh
/usr/bin/python3 -I scripts/macos-job-manifest-migration.py preview \
  --original docs/operator-packets/e579/catalog/eph-task-000001.json \
  --inventory docs/operator-packets/e579/catalog/slots.json \
  --launch-fields "$LAUNCH_FIELDS" --launch-fields-sha256 "$LAUNCH_FIELDS_SHA256" \
  --reviewed-commit "$REVIEWED_COMMIT" --operator "$OPERATOR" \
  --ledger-snapshot "$LEDGER_SNAPSHOT" --ledger-sha256 "$LEDGER_SHA256" \
  --synthetic-request-id "$SYNTHETIC_REQUEST_ID" --max-attempts "$MAX_ATTEMPTS" \
  --item-readback "$ITEM_READBACK" --item-readback-sha256 "$ITEM_READBACK_SHA256"
```

After preview and operator review, repeat these same explicit inputs with
`sudo /usr/bin/python3 -I scripts/macos-job-manifest-migration.py expand`.
It shares the launcher's `used/.lock`, checks the exact old catalog pins,
complete0600 account receipt and disabled native identity, refuses any used
51001/principal receipt, and checks workspace0700/settings0600 owned by51001.
Create the state paths only within the operator's reviewed run, as their own
accounts, using exclusive creation. An occupied path stops this sequence; do
not recursively chown or adopt it. These commands contain no model/provider
configuration and create no metadata store or capability:

```sh
sudo -u _stctl /bin/mkdir -m 700 /var/lib/shantytown/controllers/_stctl/root
sudo -u _stjobs /usr/bin/python3 -I -c 'import os; from pathlib import Path; h=Path("/var/lib/shantytown/agents/crew-eph-task-000001"); (h/"workspace").mkdir(mode=0o700); fd=os.open(h/"settings.json",os.O_WRONLY|os.O_CREAT|os.O_EXCL|os.O_NOFOLLOW,0o600); os.write(fd,b"{}\n"); os.fsync(fd); os.close(fd)'
```

The migration itself does not create/adopt these job-owned paths. Metadata and policy must
still be absent. A different UID/home/root/principal burns this slot and needs
a separately provisioned new slot; it is never repaired by this script.

The original manifest is retained byte-identically root0444 at
`jobs/migrations/archive/eph-task-000001.e985f7f8.json`, and the old slots file
is retained alongside it. The subdirectory prevents the launcher's root-level
`eph-*.json` glob from treating an archive as a capability. Original receipt
fields remain unchanged; only a `migrations` array is added, with append-only
prepared/complete events containing old/new hashes, reviewed commit, operator,
UTC time and input-evidence pins. Every earlier event is retained. Manifest and
slots are each replaced atomically **after** the durable prepared event; there
is no cross-file filesystem transaction. An interruption remains incomplete
and refuses rerun/rollback: retain all files for explicit operator recovery.
Never rerun the original provisioner's exact-receipt verifier as if this expanded
receipt were its original schema; inspect the retained identity fields and this
migration journal, then use the pinned runtime's full catalog verification.

The physical order is controller -> runtime staging as needed -> public CA
checks -> phase-A refusal -> explicit 1b expansion -> metadata and policy ->
own-key enrollment -> identity and negative controls -> normal Chaski retry.
This two-phase dependency prevents a single linear unattended setup. The successor runtime pins Seeds 0.14.0 with the
CA-file configuration and released ST `beb5d218` (v0.16.0), including the protected macOS `/var`
alias handling and canonical `/private/var` controller-root requirement. This
closes the old input-pin gaps; it does not prove native lifecycle success.
Stop before synthetic execution until this new runtime pin has completed the
normal source review, release and native acceptance.

2. **Immutable runtime**, before metadata: stage regular files with exact
pins, never `extractall`, a root download/build, or a recursive copy of an
unverified user tree. The staging helper retains interrupted `.pending` state.

```sh
sudo /usr/bin/python3 -I scripts/macos-job-runtime-stage.py \
  --archive "$ARCHIVE" --archive-sha256 "$ARCHIVE_SHA256" \
  --manifest-sha256 25cadd5a13a3f8021f9fb11bedfb789e815b5cb10141962f4fdba9ca35fb85f1
sudo '/Library/Application Support/Shantytown/bin/tambo-controller-e579' job-runtime \
  --directory '/Library/Application Support/Shantytown/runtime-artifacts/25cadd5a13a3f8021f9fb11bedfb789e815b5cb10141962f4fdba9ca35fb85f1' \
  --manifest-sha256 25cadd5a13a3f8021f9fb11bedfb789e815b5cb10141962f4fdba9ca35fb85f1 --apply
```

The existing runtime installer creates missing `/usr/local/libexec` through its
protected root-owned0755 directory helper; it refuses unsafe parents. It installs
`/opt/shantytown-job-launcher/bin/python3`, `app/`, `lib/`, `bin/sd`, and the fixed
`/usr/local/libexec/st-job-launcher`. Installed paths **drop** the source artifact's
`runtime/` prefix. Repeating exact `job-runtime --apply` verifies installed file
inventory and hashes; a different existing runtime refuses, never upgrades in place.

3. **Job metadata**, only after explicit step1b completed and the private
launch catalog is admitted at the fixed path. Missing fields must stop
preparation. The committed provision-only catalog is retained as historical
input, never presented as a launch capability.

```sh
sudo '/Library/Application Support/Shantytown/bin/tambo-controller-e579' job-metadata \
  --directory '/Library/Application Support/Shantytown/jobs' --apply
```

Read back the helper's exact `PREPARED <manifest-pin> CREATED|UNCHANGED` result and
public own-job metadata. Runtime delivery must precede this call: it invokes the
fixed helper. Preview ordering is not apply ordering.

4. **Job sudo policy**, same complete catalog, after metadata success:

```sh
sudo '/Library/Application Support/Shantytown/bin/tambo-controller-e579' job-policy \
  --directory '/Library/Application Support/Shantytown/jobs' --apply
sudo /usr/sbin/visudo -c
sudo /usr/bin/sudo -l -U _stctl
sudo /usr/bin/sudo -l -U _stjobs
```

Create-only `st-job-slots` is validated by immutable `visudo`. Do not edit,
replace or remove an existing unrelated sudoers fragment. Effective listings
must grant only helper operations for the exact principal/digest: controller
prepare/check/launch/stop/poll; job submit only; `NOSETENV` and a separate PTY.
An unrelated broad rule, including an administrator's existing passwordless
rule, must grant nothing to either dedicated account. A mismatch stops here.

5. **Own-key enrollment and CA trust — native acceptance remains required.**
The successor pins published Seeds 0.14.0 and the ST identity pass-through.
Use `[quipu] ca_file` and `ca_sha256` in the protected per-principal Seeds
configuration. Verification stays enabled: wrong/missing/tampered CA and wrong
hostname must refuse. Do not invent a CA environment variable, expand system
trust or fall back to HTTP. The new manifest is a separate reviewed artifact
revision; never patch installed runtime bytes in place.

The private host packet supplies endpoint, controller principal, introducer,
public CA bytes/provenance/hash and exact root-owned0644 CA path. Install public
CA material create-only for sd; preserve it on rollback. Each principal must
run `sd key init --session <its-principal> --agent <its-principal>
--introducer <reviewed-introducer>` **as its own UID in its own home**. Use the
existing privileged introducer through a reviewed one-shot launchd job with
`UserName`/`GroupName` set to that exact account. No model or controller receives
a general sudo impersonation rule. Only public key output goes to the existing
introducer for `quipu attest register`; no automatic `VerifierRegistration` and
no private-key copy between accounts. Capture own-key success, wrong/missing-key
and replay refusal with temporary synthetic credentials before policy activation.

Generate each key **before** redirecting that UID's config. Prepare one distinct
root-owned, non-writable configuration directory per principal; each contains
`seeds/config.toml` with the CA path/hash and that UID's own `signing_key_file`,
`signing_session` and `signing_introducer` returned by its enrollment. The key
stays in its original private home. A CA-only redirected config silently loses
the signing context and is not an acceptable packet. Each config directory and
public CA must pass ST's protected-path checks. After redirect, `sd key init`
as that UID must refuse without changing its key or config; retain this negative
control alongside own-key and cross-UID signing proofs.


`org.shantytown.controller.plist` is a disabled system-launchd proposal, not
an installed/armed daemon. Its fixed interpreter/application run `st tend` as
`_stctl` in that UID's private store. Installing it root:wheel0644 leaves
`Disabled=true`, `RunAtLoad=false`, `KeepAlive=false`; do not bootstrap, enable
or kickstart it in this packet. A later readiness decision and reviewed controller
store/identity inputs are required before arming. Its private interpreter may
also need separately measured native network permission; an interactive test is
not evidence for launchd networking.

6. **Identity policy**, after step5 proofs, never before. Render the exact
private candidate with the renderer and private inputs, record its SHA256 and
attach the complete file to the operator request:

```sh
python3 scripts/render-macos-controller-identity.py \
  --controller-principal '<reviewed-controller-principal>' \
  --endpoint 'https://graph.example.invalid' --output PRIVATE-identity.candidate.json
```

The example endpoint is a fixture, not the native target. The real candidate is
private and supplied before ASK, never guessed from the example. Complete the
rendered candidate with a distinct absolute `seeds_config_dir` on **each** UID
binding, selecting its own protected config directory from step5. The renderer's
baseline output without those fields is not CA-ready. Record the SHA256 only
after completing these private fields; review the complete bytes before delivery.
It maps51000 and51001 to their distinct principal/home, HTTPS endpoint and trusted
immutable `/opt/shantytown-job-launcher/bin/sd`. Create-only delivery is root:wheel0644 at
the fixed platform identity path, after checking exact candidate bytes/pin and
safe parents. No `required:true` installation or server enforcement occurs from
this source PR. Preserve any occupied different file for explicit migration.

7. **Native negatives, then STOP**. Capture own-UID0600 read success paired
with cross-UID read/signing denial; job cannot read controller or UID501 homes,
signal controller processes, or impersonate another principal. Failed privilege
drop never runs as controller/501. Retry without a fresh capability refuses.
Stop targets only the intended UID. Exercise both harness/write paths using
synthetic credentials, effective sudo listings, wrong manifest/identity/key and
replay negatives. A cancelled/missing/unavailable check is not PASS.
`unattended_uid_ready` remains false; arming is a separate decision.

## Rollback, reverse order and bounded scope

Retain a per-step receipt, the prior public config and exact hashes. Do not run
later rollbacks after an earlier refusal; preserve drift for explicit recovery.

- Step7/6: disable future admission/controller activation first through the
  separately reviewed host rollout. Keep strict identity enforcement installed;
  removing it can restore compatibility writes. Any identity-policy replacement
  is its own pinned migration, not a generic file deletion.
- Step5: introducer handles only the scoped public bindings after accounting for
  active jobs/sessions. Preserve private keys, homes and audit data. Do not copy
  credentials or silently revoke unrelated/active bindings. Public CA may remain.
- Step4: withdraw only the exact installed `st-job-slots` fragment to a protected
  archive outside included sudoers paths, validate complete sudoers and effective
  denial, and preserve the unrelated administrator fragment. This needs its own
  reviewed native rollback receipt; no wildcard sudoers edit is authorized.
- Step3: retain metadata and reservations; no execution grant is in public own-job
  metadata. Never recycle its UID or delete an active job's store.
- Step2: leave staged and installed immutable runtime/launcher bytes in place,
  preserving active processes. Removing files or editing a running runtime is not
  rollback. With admission/policy withdrawn, there is no new helper grant.
- Step1: after later grants are withdrawn, run the account-only tombstone script
  from the same reviewed checkout with the packet/catalog/tool pins shown above:
  use this exact account-only command (no later-grant rollback is implied):

  ```sh
  sudo /bin/sh scripts/macos-controller-account-rollback.sh \
    --packet docs/operator-packets/e579/controller.json \
    --packet-sha256 aff7d04c3b67f6cb068c6ee92c3f02012ed724a7d55b6138fc68036c6874dd9c \
    --inventory docs/operator-packets/e579/catalog \
    --inventory-sha256 21d6283211c709dd3370bf558b5c2e69529a59849b63d48d5b1af1b6c7b5c10a \
    --tool-sha256 35c646c9090cfbf29679ac8bc2754754f49454b1213a98ed1fd37d5c0446436a
  ```
  It verifies exact native state, writes an idempotent0600 root-owned tombstone,
  leaves the already disabled account disabled, and preserves UID/home/receipt.
  It refuses drift/partial provisioning and cannot undo later grants by itself.

The source task ends only after reviewed merge, exact-main green CI, published
artifact/pin, and the native operator's unprivileged packet/inspect confirmation.
It never implies native sudo apply or complete parent-workstream acceptance.

### Step1b rollback boundary

Only before metadata/policy and before any launch receipt: use the same pinned
inputs as step1b, replacing `expand` with `rollback`. A matching complete
expansion journal is required; the original manifest/slots are restored exactly
and rollback events are appended without erasing history. The account stays
disabled and reserved. After a launch, metadata/policy apply, an interrupted
migration or any drift, this rollback refuses. Never delete a consumed receipt,
reuse51001, or manually erase a prepared event to make a rerun succeed.
