# Frozen trixie package qualification

This manual qualification builds the pinned Tambo source in a disposable,
GitHub-hosted Debian 13 amd64 container. It neither publishes a package nor
installs anything on a developer or crew host. The workflow has no push, PR or
scheduled trigger; do not dispatch it until its source PR has complete green
CI and an independent review. The initial scope permits one dispatch. A failed
build is evidence to inspect, not permission for an automatic rerun.

The input manifest pins source `eebd1f548725403a845d60f1841f2dbfc4fd4a29`
(Tambo 0.13.4), its Cargo manifest/lock/release recipe, and the archive snapshot
`20261104T112000Z`. All three InRelease files and six source/amd64 compressed
indexes must match their recorded hashes and sizes. Index entries must also
occur in the pinned Release SHA256 section. APT independently verifies Debian
signatures and its cached Release must match before any build tools are
installed. HTTPS verification stays enabled. Historical-snapshot expiry mode
is explicit; this is not a claim that November 4 security metadata is current.

The small local JavaScript action uses the runner's supported Node 24 runtime
([GitHub metadata contract](https://docs.github.com/en/actions/reference/workflows-and-actions/metadata-syntax))
and its TLS roots to bootstrap APT without first installing prerequisites from
a live repository. It refuses ordinary host execution. No credentials are
persisted by checkout, and no repository secrets are supplied to the build.

The selected release configuration is default features with both declared
binaries; the test-only feature is excluded by the upstream release recipe.
The package build uses only the distro Cargo registry, `--offline --locked`,
and both binaries. It never deletes or updates the upstream lock or drops
features to obtain a pass. It records the effective compiler and installed
packages. Smoke tests run both `--version` commands; this is not a claim of the
upstream development-feature unit suite passing on the target distro.

The pinned upstream inventory contains 61 normal/build packages, distinct from
79 metadata packages including development dependencies. Candidate lookup in
the frozen indexes leaves four names without a candidate and 54 locked versions
without an exact candidate. These are compatibility/source-packaging obligations,
not proved missing API support or estimates of engineering hours. The lift CSV
names each obligation; the actual offline build supplies the next falsifier.
License expressions, observed file hashes and the installed distro registry are
recorded separately. They do not establish DFSG clearance or exhaustive terms
for every bundled asset.

Artifacts expire after seven days and contain bootstrap/build logs, dependency
and license evidence, compiler/package versions, the lift CSV, result receipts,
lintian logs and cleanup evidence. Source-package lintian can run even if the
binary build fails; a missing binary lintian result stays absent. The disposable
root job uses the documented
[`--allow-root` option](https://manpages.debian.org/trixie/lintian/lintian.1.en.html). Qualification
packages and binaries are not uploaded. No release or APT repository is changed.

The always-run cleanup removes both checkout and build trees and records their
absence. This proves tree cleanup only. After the single dispatch, retain the
exact run/commit, artifact hashes and terminal job inventory, and verify the
runner's container-removal log before asserting managed builder teardown.
Missing teardown evidence remains UNKNOWN; do not substitute an earlier cleanup
step for container disposal. The parent packaging effort remains open until the
remaining target and compatibility checks have measured outcomes.

Local refusal controls use no APT or Cargo:

```sh
python3 -m unittest discover -s tests -p test_trixie_qualification.py
```
