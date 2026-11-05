# Hosted installation method qualification

`install-methods.yml` runs Quipu's documented package methods on fresh
GitHub-hosted images: Homebrew on `macos-15`, apt on `ubuntu-latest`, and the
published container on a separate `ubuntu-latest` job. It does not use a live
Mac, provision a guest, or request deployment credentials.

After this source has passed CI, independent review and normal landing, the
workflow owner can select **Quipu install method qualification** in Actions, or
run:

```sh
gh workflow run install-methods.yml --repo intigraph/tambo --ref main \
  -f method=all -f trust_brew_formula=true -f container=true -f image=ghcr.io/intigraph/quipu:0.8.1
```

Select `method=brew`, `method=apt` or `method=container` to run only that
method. For the failed Homebrew leg, keep the earlier apt PASS receipt and use
`-f method=brew -f trust_brew_formula=true -f container=false`. The container
owner uses `-f method=container -f container=true` only after its separate image
gate is met. A skipped method is not re-qualified and does not erase an earlier
receipt. `method=all` retains the initial multi-method qualification.

Record the actual run's `head_sha`. A method qualifies only from its successful
job and `result.json` with `status: PASS` at the accepted landed checkout.
A green source PR or overall workflow result does not qualify a skipped job.
Do not dispatch again merely because a job is queued or running.

The default container reference is the version declared by Quipu's published
`server.json`, independently of the newest native release. The input permits
only the public Quipu image with a version or SHA-256 digest. The receipt records
the pulled immutable digest; subsequent executions use that digest. A failed
pull or unavailable image fails that leg, without substituting a locally built
image or treating registry failure as successful installation.

## Evidence and cleanup

Each package job requires an absent baseline, checks the installed commands from
the package-owned prefix or file list, runs the package manager's upgrade
command, removes the package, checks command/alias absence, and proves that an
independent project data marker survived removal. Explicit removal of that
owned marker is a separate operation. If the upgrade command is a no-op,
`upgrade_version_changed` is false; it is not a tested version transition.

Homebrew requires the explicit `trust_brew_formula=true` dispatch admission
(or `--trust-brew-formula` for the runner helper). Without it, that leg fails
before any package-manager command. The admitted leg records and runs
`brew trust --formula scbrown/tap/quipu`, never whole-tap trust or a disabled
trust check. It runs cleanup while the formula is still installed, then
uninstalls even if cleanup fails. A refusal remains fatal. Formula trust is
removed with `brew untrust --formula scbrown/tap/quipu` afterward, including
failure cleanup. See [Homebrew Tap Trust](https://docs.brew.sh/Tap-Trust).
This changes only the fresh disposable runner, never a live Mac.

The apt job checks the documented primary repository-key fingerprint before
trusting the repository. Homebrew checks the formula's `qp` alias and both
executable companions rather than accepting another binary on `PATH`.

The container job proves CLI/server versions, a real local-store write/read
across container recreation, and successful MCP initialization and tool listing
through the image's default entry point. Container removal must retain the
owned volume. Before explicit volume deletion, the job copies only its fixture
database and verifies its SHA-256 against the source bytes. Generated signing
keys are not exported. Cleanup retains the exact owned container IDs and volume
name, including when a timed-out client could leave a server running.

Per-command stdout/stderr and the checkout/digest/result receipt are uploaded
even when qualification fails. Cleanup failures make the result fail and do not
erase the original failure. The helper refuses local or self-hosted execution,
preinstalled packages, and reused report directories. Its offline tests never
invoke a package manager or Docker.

## Remaining qualification

This workflow does not claim a full Tambo apply/verify/removal result. That path
still requires its separately admitted canonical credential and reviewed plan.
No token is embedded, fabricated, borrowed from another installation or uploaded
as an artifact. The result records that qualification as **UNVERIFIED**.
