# Immutable job runtime delivery

These staged commands support the protected per-job UID launcher. They never
enable unattended admission, enroll credentials, launch a harness or restart a
service. Actual apply requires a separately reviewed native operator packet.

`tambo job-runtime --directory <artifact> --manifest-sha256 <pin>` inspects an
artifact without executing its files. Add `--apply` only in the admitted root
delivery lane. The running Tambo binary, source artifact and their parent paths
must be immutable and root-owned for apply. Linux and macOS use the same file
installer; account creation remains platform-specific.

The artifact has a `manifest.json` with exactly `version: 1`, `platform` (`linux`
or `macos`) and a `files` map of relative names to SHA-256 digests. The separately
supplied manifest pin binds the entire map. Every member must be a regular file;
symlinks, special files, traversal, extra files/directories and digest changes
refuse. Bounds are 4096 files, 8192 total entries, 256 MiB per file and 1 GiB total.

Required members include `launcher`, `runtime/bin/python3`, the Shantytown
application under `runtime/app/shantytown`, and one matching Python >=3.11 standard
library under `runtime/lib/python3.<minor>` (including `os.py` and `encodings`).
The interpreter is a regular native executable, not a venv symlink. The launcher
uses the exact private interpreter with `-BIS`; its own bootstrap requires the
private prefix, immutable import paths and an immutable application. The artifact
contains public runtime code only. Provider logins, session keys, verdict keys and
controller files are separate private delivery inputs and never belong here.
Optional regular `runtime/bin/sd`, `runtime/bin/codex` and `runtime/bin/claude`
members freeze the trusted client and native harnesses in the same protected
tree. The host manifest and identity policy must pin these paths or separately
delivered immutable executables; agent-writable installations cannot qualify.

Apply installs `/opt/shantytown-job-launcher` and the fixed
`/usr/local/libexec/st-job-launcher`. It uses exclusive staging, root-owned
normalized permissions, file/directory fsync and exact readback. A different
existing runtime/helper refuses; this initial installer is not an upgrade or
rollback primitive. Interrupted staging is retained for explicit recovery.

`tambo job-metadata --directory <protected-host-catalog>` previews account
metadata. Its separately admitted `--apply` calls only the helper's fixed
`prepare <principal> <digest>` for each declared slot. Root provisioning supplies
the controller UID from the protected manifest; preparation writes public
own-job metadata only and assumes no agent identity. Changed/unchanged replies
must match exactly. Linux uses `/etc/shantytown/jobs`; macOS uses
`/Library/Application Support/Shantytown/jobs`.

`tambo job-policy --directory <catalog>` renders the exact sudoers fragment.
Its separate `--apply` validates the candidate using immutable native `visudo`
before create-only delivery to `st-job-slots` in the platform sudoers directory.
An existing different fragment refuses; no rule is silently replaced. Each
dedicated controller UID receives only prepare/check/launch/stop/poll for its
exact principal and digest. The job UID receives only submit for itself. Rules
use `NOSETENV` and explicitly require sudo's separate PTY for this helper, so a
job cannot inherit the invoking controller's terminal for input injection. They
grant no shell, interpreter, installer or generic impersonation command. Both
controller and job UIDs must be >=10000 and distinct. An unrelated
existing sudo policy can still grant more rights; installed negative acceptance
must prove effective rights and that the fragment is actually included.

Before any apply, retain the reviewed source/binary/artifact/catalog hashes,
native platform/import-prefix proof, per-host account count/range and nonreuse
records, controller migration, own-key enrollment through the existing introducer
and registry delivery, exact private settings/workspace permissions, scoped
sudoers rules and rollback packet. After approved installed file/signal/identity
and ordinary launch/retry controls, readiness remains a separate decision.
Rollback disables admission while preserving active jobs, identities and receipts;
it never recycles a UID or edits a running runtime in place.
