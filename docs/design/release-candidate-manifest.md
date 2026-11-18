# Frozen candidate inputs v1

This offline contract is installed inert for release-discipline work package A.
It changes no timer, registry, key, publisher, channel pointer or installed tool.
The existing workflows retain their current publication behavior. Promotion
remains disabled pending the separate evidence, compatibility, breaking-record,
approval and deployment gates. A successful fixture is not a seven-day soak.

## Producer and consumer sequence

1. Commit the final per-tool version before building. Freeze a plan using the
   existing source checkout. The Tambo adapter checks a clean tree, committed
   explicit Cargo package version, full commit, SHA256 of recursive NUL-delimited
   `git ls-tree -rz --full-tree HEAD` output, and SHA256 of `HEAD:Cargo.lock`.
   It never patches a version or invokes a compiler. Write outputs outside the
   checkout so that generated inputs do not dirty the source.
2. The existing producer builds once. Its build inventory binds the frozen plan,
   source, run AND attempt, every advertised target/package, observed embedded
   version, size and SHA256, and exact named test-receipt digests.
3. Seal the inventory against local distribution bytes. Missing/extra/duplicate
   packages, version/source/run/checksum drift and output overwrite refuse.
4. The existing admitted publisher signs canonical candidate bytes with its
   existing signing path. This module neither creates keys nor signs. Verify
   detached Ed25519 provenance with the separately pinned public-key SHA256 and
   policy revision. Other signing algorithms require reviewed adapters; do not
   relabel them or mint a new publisher identity to use this interface.
5. Verify again before using the candidate. Optional same-byte promotion inputs
   preserve the identical artifact rows and candidate ID, but explicitly state
   `authorization: NOT_ADMITTED`. They do not switch discovery or publish files.
   Work package B must additionally enforce soak, real platform and restore
   receipts, current breakage inventory, compatibility, approval and prior-state
   generation locking immediately before any promotion.

```text
python3 scripts/release_candidate.py freeze plan.json frozen.json --checkout SOURCE
python3 scripts/release_candidate.py seal frozen.json build.json DIST candidate.json
python3 scripts/release_candidate.py verify candidate.json DIST candidate.sig publisher.pub KEY_SHA256
```

Append `--promotion-inputs inputs.json` to verification for an inert input file.
The tool performs no downloads, uploads, builds, installs or registry actions.

## Wire identity

Canonical JSON uses sorted keys, ASCII escapes, compact separators, and a final
newline. Duplicate keys, nonfinite numbers and unknown fields refuse. List order
is significant, especially for breaking records. No self-referential digest is
hashed into itself.

`release-plan/v1` fields:

| Fields | Required binding |
| --- | --- |
| `tool`, `repository`, `version` | Existing identity; final major.minor.patch, never rewritten rc/nightly bytes |
| `source` | Full commit, tree_sha256, lockfiles_sha256 |
| `build` | Immutable run_id string, positive attempt, workflow_sha256 |
| `targets` | Unique OS/architecture rows; each has named distributions with kinds |
| `toolchain`, `dependencies` | Nonempty named SHA256 inventories |
| `compatibility_sha256` | Exact compatibility-set document |
| `breaking_records` | Ordered unique `{id, sha256}` file bindings, empty only for nonbreaking candidates |
| `previous_stable_sha256`, `restore_plan_sha256` | Previous artifact set and inverse/restore plan |
| `capabilities` | Explicit store/pendant read/write, CLI and config version lists |
| `feature_profile`, `experimental_features` | Supported profile and named experimental paths |
| `required_receipts` | Exact nonempty named test receipt inventory |
| `verification_policy_sha256` | Independently pinned verifier policy revision |

Distribution kinds are archive, wheel, crate, vocabulary, library and pendant.
No binary is invented for a vocabulary. Unsupported distributions must not be
advertised. Tools without a given format use a reviewed explicit capability
marker such as `not-applicable`, never an empty guessed compatibility range.

`frozen-release-plan/v1` contains plan and plan_id. The latter is SHA256 of the
domain `quipu-stack/release-plan/v1`, one NUL byte, and canonical plan bytes.

`candidate-build/v1` contains plan_id, exact source/build, receipts (name-to-SHA256),
and artifacts. Each artifact records os, architecture, name, kind, public HTTPS
url, positive size, sha256 and producer-observed embedded_version. URLs are
locators only: the offline tool does not establish remote immutability. They
must carry no credentials, query string or private inventory.

The envelope contains candidate_id and payload. Payload has schema
`release-candidate/v1`, experimental=true, frozen, and build. Candidate ID is
SHA256 of domain `quipu-stack/release-candidate/v1`, one NUL, and canonical
payload. It is channel-independent. Detached signature covers the canonical
**whole envelope**. A final version string never removes experimental=true.

This binding does not prove chronology by itself. The trusted producer must
freeze before building and retain its native run evidence. Likewise, embedded
versions are observations supplied by that producer's existing binary/package
verifiers; hashing a caller's string is not independent package inspection.
The separately admitted signing identity attests that complete inventory.

## Integration without duplicate pipelines

Quipu xxoa.8 already provides scripts/ci/nightly-result.py, exact-SHA green,
immutable commit/run prereleases and overwrite refusal. Wire this contract into
that existing producer before build and beside its existing experimental assets;
do not add another nightly publisher. Its current rolling archives without the
final-version/provenance contract are not automatically promotable stable bytes.

Tambo's existing release.yml target matrix, source-version check,
check-release-assets.sh, crate packaging and repository-pendant verification
remain the source of outputs and verification receipts. Work package B must
move package creation before soak and use frozen files at promotion, rather than
rebuild on the stable tag. This source slice activates neither that conversion
nor a new signing path; its only CI change runs offline controls.

c7yh retains preview adapters, ro425e.16 retains pendant contracts. Pendant bytes
may be advertised artifacts and their verified receipts are required inventory
entries. Compatibility and breaking-record producers supply real digests/IDs;
missing proofs must not be filled with synthetic release claims. Each breaking
record entry binds its exact file SHA256 and ID, preserving order. Work package
C's release validator must check actual record bytes and release admissibility;
a nonempty digest here alone does not graduate a draft record. Other source
repositories need reviewed committed-version adapters; the CLI only implements
Tambo's explicit Cargo form and refuses to pretend it fits another repository.

## Acceptance still owed

Offline controls cover actual detached signatures, corruption/source/build/
version/target refusal, no overwrite, and unchanged promotion input bytes.
Ephemeral signing keys exist only inside test fixtures. No production key,
Cargo build, native install, real scheduled candidate or promotion was performed.

Work package A stays open until existing producers emit this manifest and
detached provenance via reviewed inert integrations. This contract slice alone
does not complete whole-stack producer or runtime acceptance.
