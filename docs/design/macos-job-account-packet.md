# Steve-run macOS account packet (e579)

The setup script wraps the existing merged `job-slots --apply` provisioner.
It creates only the reviewed disabled-login account(s), private homes and durable
nonreuse receipts. It installs a pinned root-owned copy of Tambo and the
reviewed catalog under `/Library/Application Support/Shantytown`. It does not
install sudo rules, enroll keys, create controller accounts, launch jobs or change
`unattended_uid_ready`. Steve runs the sudo commands; crew agents do not.

Before submitting the ASK, Hammond attaches the exact reviewed script commit,
Tambo executable SHA256, catalog SHA256 and complete catalog/manifest files.
The catalog transitively pins each manifest. Use the already accepted per-job
slot schema in [job-slots.md](job-slots.md); one initial account may be named
`_stjobs`, with later slots named `_stjobs_<suffix>`. Never share one job UID among
multiple principals. UID/GID must be at least10000, unique and never recycled.
The distinct protected controller UID in each manifest must also be at least10000:
Steve's login UID501 is not a substitute. This account-only packet does not
migrate the controller or qualify its later launch capability.

The reviewed inventory must contain `slots.json` plus the manifests it pins.
A tool built from normal reviewed source or a normal published artifact is an
input; the script does not download, build or choose a moving release as root.
Run these from the exact reviewed checkout (the paths and two pins are concrete
values attached to the ASK, not values fetched by the script):

```sh
sudo /usr/bin/python3 -I scripts/macos-job-account-packet.py setup \
  --tool /absolute/path/to/reviewed/tambo --tool-sha256 TOOL_SHA256 \
  --inventory /absolute/path/to/reviewed/catalog --inventory-sha256 CATALOG_SHA256
sudo /usr/bin/python3 -I scripts/macos-job-account-packet.py verify
```

Setup validates pins before writing. Native whole-plan validation and collision
checks happen before account creation. Existing paths are accepted only with
identical bytes, root ownership and exact modes; foreign accounts/UIDs/homes are
never adopted. Rerunning a completed setup is safe. An interrupted native apply
retains prepared UID reservations and refuses automatic recovery; keep the exact
failure and obtain explicit recovery rather than delete a receipt or recycle a
UID. Python3 must be available at the displayed native path; no root package
installation is performed.

Verification reads native UID/GID uniqueness, hidden/disabled login and false
shell, negative admin membership, home UID/GID/mode0700, and complete root-owned
nonreuse receipts. It never launches a process as the job account or imports job
code. A successful verify is account provisioning evidence, not native job
isolation or unattended readiness acceptance. Capture command exit codes and
sanitized output on e579; no key material belongs in this packet.

Rollback of this **account-only** stage:

```sh
sudo /bin/sh scripts/macos-job-account-rollback.sh
sudo /usr/bin/python3 -I scripts/macos-job-account-packet.py verify
```

Rollback verifies the exact installed identities, then writes an idempotent
root-owned tombstone preventing this setup packet from running again. Accounts
were already disabled and no execution/admission grant was installed by setup.
It preserves accounts, homes, receipts and any job data; it does not stop jobs,
delete identities, revoke unrelated policy or undo a later runtime rollout.
If runtime/policy/key activation has subsequently occurred, use that separately
reviewed rollout's rollback first; this tombstone does not revoke those grants.
A drifted or partial account refuses rollback and retains evidence for explicit
recovery. Removing the tombstone to retry is not an automatic operation.

Existing source tests use disposable files and fake command replies. They do not
establish Darwin mutation or live native isolation. Native setup/verify/rollback
still require Steve's actual run and receipts. Full e579 stays open and readiness
false until separately admitted controller/key/policy/runtime delivery and
ordinary launch/retry/impersonation negative controls pass on each host.

## Proposed one-account ASK inputs

The committed `docs/operator-packets/e579/catalog/` proposes exactly one initial
slot: principal `eph-task-000001`, username `_stjobs`, UID/GID51001 and protected
controller UID51000. Both UID values are proposed for Steve's ASK, not evidence
of a registered controller or granted live job. The account inventory read on
this Mac found Steve atUID501; it did not create any record. Native collision
checks run again at setup. This first slot can never be recycled for a second
job. A later protected controller migration/manifest expansion needs its own
reviewed packet. No native launch is possible from this provisioning-only
manifest, which deliberately contains no command/key/runtime grant.

Concrete setup from the reviewed checkout using the already verified native
Tambo0.7.1 compatibility artifact (installed under its legacy executable name) (SHA25629c29ceef6ac89728686cd8922bf0e142dc8200fa6067b171b05d45e38fc397c):

```sh
sudo /usr/bin/python3 -I scripts/macos-job-account-packet.py setup \
  --tool /absolute/path/to/reviewed/tambo \
  --tool-sha256 29c29ceef6ac89728686cd8922bf0e142dc8200fa6067b171b05d45e38fc397c \
  --inventory docs/operator-packets/e579/catalog \
  --inventory-sha256 21d6283211c709dd3370bf558b5c2e69529a59849b63d48d5b1af1b6c7b5c10a
```

If that existing tool was upgraded, pin and review the new exact artifact first;
the script will refuse the old pin rather than download or select a replacement.
