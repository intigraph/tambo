# Protected job account slots (e579 source)

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
