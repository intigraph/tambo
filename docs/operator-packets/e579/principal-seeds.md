# Own-UID key initialization and protected Seeds config candidates

Source packet for aegis-e579. Rendering never invokes `sd`, `launchctl`, sudo,
account commands, an enrollment service or a model. No private key is an input
or an output. The private host packet and native execution remain with the
operator; readiness remains **FALSE**. This packet is not permission to run it.

The approved sequence is **controller -> runtime -> phase-A refusal -> explicit
1b expansion -> metadata -> policy -> ownkeys -> identity**. In particular,
ownkeys cannot move ahead of metadata/policy. The public CA-only checks already
performed are distinct from own-key enrollment. See [phase A](phase-a.md).

## Render before native admission

Use the exact reviewed Tambo commit. Save private inputs with mode0600. Select
one account from the reviewed controller/catalog; never reuse an account or
substitute a login UID. The renderer supports only these two existing packet
identities; another job slot requires a separately reviewed catalog/renderer.

A controller fixture input for the first phase is:

```json
{
  "username": "_stctl", "uid": 51000, "gid": 51000,
  "home": "/var/lib/shantytown/controllers/_stctl",
  "principal": "controller-fixture", "introducer": "operator-fixture"
}
```

Replace fixture identifiers with the **admitted private** controller principal
and configured introducer. For the first job use `_stjobs`, UID/GID51001,
home `/var/lib/shantytown/agents/crew-eph-task-000001`, principal
`eph-task-000001`. The live packet's introducer is the administrator named in
the item's ruling; never choose it from an environment variable or guess it
from the account name.

```sh
/usr/bin/python3 -I scripts/render-macos-principal-seeds.py key-init \
  --input PRIVATE-key-init.json --output PRIVATE-key-init-candidates
```

The output is a **disabled**, non-restarting, non-RunAtLoad system launchd plist,
with explicit UserName and GroupName, own HOME, own XDG_CONFIG_HOME, umask077,
and the fixed immutable `/opt/shantytown-job-launcher/bin/sd key init --session
PRINCIPAL --agent PRINCIPAL --introducer INTRODUCER` argv. No shell, arbitrary
program, inherited provider environment or redirected protected config is used.
The renderer refuses an existing output directory, including a symlink. It
prints hashes, not private inputs. Candidate directories/files are0700/0600;
this is private staging, **not proof of root ownership or installation**.

## Later native one-shot operation: separately admitted, held here

Only when the owner-acknowledged order reaches ownkeys and the native packet is
admitted, the operator checks the complete plist/hash and account receipts,
installed runtime pin, effective groups and private home. Install the exact
reviewed plist create-only as root:wheel0644 under `/Library/LaunchDaemons`.
No existing job is adopted/replaced. Record the launchd label and exact plist
hash. Do not load the ordinary controller daemon.

Before loading even the one-shot job, verify every component of
`/Library/Logs/Shantytown/e579` is root-owned and not group/other writable;
create missing directories as root:wheel0755 and the exact two per-account
output files create-only as root:wheel0600, with no symlinks, hardlinks or
occupied files. launchd opens these paths with privilege: placing output in a
job-writable home would permit a symlink attack. Retain refusal on any drift.
Inspect the account's own default Seeds directory and refuse any occupied key
or signing configuration; never remove a key to make init succeed.

The native operator's admitted operation explicitly enables, bootstraps and
kickstarts **only this one-shot label**, waits for its actual LastExitStatus,
then bootouts/disables that label. Disabled=true means a bootstrap alone is
not success. Capture the exact commands, UTC times, launchd UID/GID, exit0,
public-only stdout, error output, own-home key owner/mode0600 and config owner.
A missing/cancelled/job-not-run result is UNKNOWN. No retry or second session
is implicit: an ambiguous init may have created its key; inspect without
printing private key bytes. Preserve the key and receipts on failure/rollback.
Only the admitted introducer registers the **public** output with Quipu; this
packet neither calls `quipu attest register` nor grants verifier authority.

## Render the post-enrollment protected configuration

After actual own-UID enrollment, use its returned signing values. Add these
fields to that account's original private JSON (no fields may be omitted or
added; a CA-only redirect refuses):

```json
{
  "endpoint": "https://graph.example.invalid",
  "graph": "https://example.invalid/project/fixture",
  "ca_file": "/Library/Application Support/Shantytown/ca/public.pem",
  "ca_sha256": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
  "signing_key_file": "/var/lib/shantytown/controllers/_stctl/.config/seeds/keys/controller-fixture.key",
  "signing_session": "controller-fixture",
  "signing_introducer": "operator-fixture",
  "seeds_config_dir": "/Library/Application Support/Shantytown/seeds-config/_stctl"
}
```

These are fixtures, not native CA bytes, fingerprints or credentials. Do not
infer a real key filename from the example; use the enrolled account's output.
The job uses a distinct `seeds-config/_stjobs` directory and its own key path.
Endpoint credentials, HTTP fallback, another UID's key path, traversal,
incorrect session/introducer and an unprotected config destination refuse.

```sh
/usr/bin/python3 -I scripts/render-macos-principal-seeds.py config \
  --input PRIVATE-enrolled-config.json --output PRIVATE-config-candidates
```

Read and hash `seeds/config.toml` and `installation-requirements.json` before
native admission. At the admitted native step the operator delivers each
candidate create-only at its recorded seeds_config_dir, root:wheel0755 for
the directory and `seeds/`, root:wheel0644 for config.toml. Verify safe parents,
no symlinks/hardlinks, exact bytes, distinct directories and the existing public
CA pin. Leave the private key in its own UID's home; do not copy or chown it.
The JSON explicitly reports installed=false and readiness=false until native
proof is attached. Copy the rendered seeds_config_dir into that UID's complete
identity binding and review the final identity candidate/hash before delivery.

Retain own-UID signing positive; cross-UID read/signing denial; bad/missing key,
wrong/tampered CA and hostname negatives; and `sd key init` after protected
redirect refusing without changing key/config. A rendered config is not any of
these proofs. No readiness or identity enforcement is activated here.

Rollback: prevent another one-shot execution, bootout/disable only its exact
label, retain public logs/config and private key in place; any binding revocation
is the introducer's scoped admitted operation. Never recycle the UID/session,
copy keys, delete a prepared receipt, or remove strict identity to recover writes.
