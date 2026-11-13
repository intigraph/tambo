# Protected job account slots

`tambo job-slots --directory <reviewed-directory>` reads an explicit Shantytown
`slots.json` and its exact hashed manifests. It prints account metadata only.
The range starts at UID 10000 or higher, contains at most 1024 declared slots,
and assigns one principal, username, UID and primary GID to each slot. There is
no allocator, automatic growth, account adoption, or recycling path.

The separately authorized macOS operation adds `--apply` and requires the fixed
root-protected `/Library/Application Support/Shantytown/jobs` directory and an
immutable root-owned Tambo executable. This is an account-provisioning component,
not an activation command. Linux account delivery belongs to Goldblum.

Before creating any account, apply rejects occupied usernames, UIDs, groups and
homes and durably reserves every proposed UID under `account-receipts/`. A partial
failure retains its prepared receipts and requires explicit reviewed recovery.
Rerunning accepts only complete receipts with the same manifest digest and exact
native account readback. Never remove receipts to make a UID reusable.

Accounts have their own primary group, a disabled password/authentication
authority, `/usr/bin/false`, a hidden macOS record and a private home. The command
checks account properties and negative admin membership before completing each
receipt. A timeout or ambiguous native response fails the operation.

This source does not install the privileged job helper, enroll actor credentials,
register verdict keys, deliver job settings/workspaces, launch a job, restart a
service, or set `unattended_uid_ready`. Those remain separate reviewed delivery
steps. In particular, do not copy a controller key or writable controller store
into a job home.

Before any host apply, retain the exact source and manifest hashes, approved
per-host count/range, current account/group/home inventory, controller migration
prerequisites, operator authority and rollback packet. Rollback disables further
admission while preserving identities, receipts and active-job state; it never
deletes a receipt to reclaim a UID. Readiness stays false until installed native
macOS and Linux isolation, impersonation negatives and ordinary launch/retry
controls have independently passed.

Private Linux qualification covers six account-plan/parser/platform controls and
all-target/all-feature clippy. It does not establish Darwin account mutation or
native job isolation. No live account operation was run to qualify this source.

## Expanding an inventory without disabling the existing slot

The launcher validates the identity, protected bytes, catalog hash and isolation
of every slot. A slot may have exactly the eight provision-only identity fields
or the complete identity plus launch fields. Provision-only slots participate in
UID, account, controller and hash collision checks but cannot be selected or
passed directly to any privileged launch operation. A partially populated launch
manifest still refuses the inventory. The helper's direct-operation checks stay
strict; there is no caller-provided argv or request substitution.

Generate per-slot frozen launch artifacts with the matching Shantytown release:

```sh
python -m shantytown.job_manifest \
  --directory reviewed-catalog --catalog-sha256 CATALOG_SHA256 \
  --bindings per-slot-bindings.json --bindings-sha256 BINDINGS_SHA256 \
  --output new-catalog
```

The bindings object maps explicit slot principals to all eight launch fields:
`controller_root`, `workspace`, `settings`, `harness`, `request`, `argv`,
`environment` and `review`. Each request is frozen independently; no request or
command is inferred from the controller's live environment. Existing complete
capabilities and unbound identities are copied byte-for-byte. Identity edits,
replacement of an existing capability, duplicate keys, partial fields, invalid
requests and changed input hashes refuse before output. Output is a new private
artifact directory, never an installed catalog or account operation.

Review-mode account output reports `launch_fields_complete` (field-set shape,
not native readiness). Metadata preparation and permission rendering skip
provision-only slots. Complete capabilities still require the launcher's full
value validation and native admission. Re-render the catalog after the initial
launch-field migration; preserve the original working slot and its receipts.

## Exact policy replacement

`job-policy --apply` remains create-only unless the separately authorized command
also supplies `--replace-sha256 OLD_POLICY_SHA256`. Replacement requires the exact
current predecessor bytes and mode, validates the staged candidate with native
`visudo`, archives the predecessor as `.st-job-slots.previous.<sha256>` and checks
the predecessor again before atomic installation. The archive's dotted filename
is not another active sudoers fragment. Concurrent/unknown changes, failed syntax
validation, or retained staging refuse and preserve evidence. A no-change apply
is idempotent. Restore a prior reviewed catalog and replace the current policy
using its exact current hash for rollback; retain the archive and all receipts.

## Explicit interrupted-batch resume

`job-slots --apply --resume` admits only reservations with the same principal,
UID and manifest hash. It verifies completed accounts without recreating them,
continues untouched prepared slots, and can complete the receipt for a fully
verified reserved account after a lost completion write. Every fresh UID is
reserved before the first native creation. A partially created native identity,
collision, changed reservation, unknown readback or incomplete migration still
refuses; the tool never blindly replays a partially executed account sequence.
A native partial identity therefore needs a separately reviewed recovery rather
than account adoption or receipt deletion.

The original account receipt may retain its provision-only hash and a completed
launch-field migration journal. Account verification follows only complete,
matching prepared/completed migration pairs to the current manifest hash; it
preserves the receipt bytes. A partial journal remains unknown. This allows
migration before expansion without recreating the original account.

Unit fixtures cover a batch interrupted after the first account, explicit resume
without recreating that account, a lost completion write, partial native-state
refusal, changed reservations and migration-chain mismatches. These are source
controls, not proof that any account or policy was changed on a host. Installing
both reviewed releases and native platform acceptance precede account expansion.
