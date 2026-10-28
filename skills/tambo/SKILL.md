---
name: tambo
description: Install, verify, diagnose, resume, or update the Quipu-stack tool corpus with Tambo. Use when an agent or human wants a review-first guided installation of Quipu, Camayoc, Bobbin, Yupana, and Desire Path, needs to resume an interrupted Tambo interview or apply, or needs functional proof that the installed stack works.
---

# Tambo

Install, inspect, resume and verify the stack through the `tambo` CLI.
Preserve the plan and state paths below.

Use the local quickstart for a default install. Customized installs follow
interview → plan review → apply → verify. Use the same CLI in every harness.

## Bootstrap and preflight

For Tambo itself, prefer the reviewed checksummed release path documented in
`docs/book/src/installing.md`. A source install must pin a reviewed commit.

1. If `tambo` is absent and the request authorizes installation, install the public source with:
   `cargo install --git https://github.com/intigraph/tambo --rev <reviewed-commit> --bin tambo --locked`.
   Otherwise stop and request authorization; bootstrapping Tambo is itself an
   installation.
2. Prove the binary answers: `tambo --version`.
3. Run `skills/tambo/scripts/check-surface.sh "$(command -v tambo)"`
   when this skill directory is available. A failed surface check is a bootstrap
   failure; do not continue with guessed commands.
4. For an existing or customized plan, run `tambo doctor` before apply or
   install. Without a plan doctor checks all tools, so it is not a prerequisite
   for the default quickstart. It is read-only. Report every `FAIL` line with its fix and
   stop until each is resolved; report `warn` lines to the operator.

Do not print credentials or introduce private endpoints. Do not delete
`.caboodle/` to recover from an error: it contains the resumable interview and
last-proven state.

## Default local install

When installation is already authorized, use a persistent empty work directory
and run `tambo quickstart`. It saves a retrieval plan, installs Quipu, Camayoc
and Bobbin, and verifies each with isolated functional controls. There is no
interview, issued token or running-server prerequisite. Recheck with
`tambo verify`; keep `caboodle-plan.toml` and `.caboodle/` to resume.
Quickstart reuses an existing plan and never overwrites it.

Remote access is advanced and opt-in: `QUIPU_SERVER` or a plan's `quipu_mcp`
selects authenticated remote verification. That path still requires an issued
credential, canonical permissions, and an accepted server probe. Never invent
or auto-mint an issued credential. Local signing-key enrollment is not an
installation prerequisite.

The interview below is for customized profiles and intended-use contracts;
it is not required for the default single-user install.

## Interview and plan review

For a guided install, run:

```sh
tambo init --guided
```

Run from the same working directory throughout: default session, plan, and
state paths are relative. Before resuming, inspect `.caboodle/interview.toml` to
confirm the accepted answers. An older `caboodle-plan.toml` or state file is not
evidence that this interview completed. If custom paths or the original working
directory are unknown, stop and ask; do not search broadly or start a new
session that could fork the plan.

If input ends, rerun the exact command. Accepted answers resume from
`.caboodle/interview.toml`; no install has happened. For automation with choices
already supplied, use `tambo plan --profile kg|retrieval|code-intel|everything`. Crew plans may be
generated with `--profile crew --crew shantytown|creel|both|standalone`.

The interview must elicit intended use, each desired crew member's theme/domain/
role, and at least one question the finished ontology must answer. For an
unattended run, write the same reviewed schema to `caboodle-intent.toml` and run
`tambo plan --intent caboodle-intent.toml`; do not substitute install flags
for the use interview.

Read `caboodle-plan.toml` back to the operator. Confirm the selected profile,
tools, crew themes, and anticipated-question contracts. Never apply a
conversational answer directly: the plan file is the
review boundary. If the request did not already authorize installation, stop
after presenting the plan. Do not run `apply`, `install`, `verify`, or
`project-settings` at that boundary: verify performs functional writes and
updates state, while settings projection writes configuration artifacts.

## Apply and verify

After plan approval:

```sh
tambo apply
tambo verify
tambo verify-questions
```

`apply` proves each installed binary by version read-back. `verify` then runs an
isolated negative control, functional write/index, and reader-path proof for
each selected adapter. Both commands update `.caboodle/state.json` atomically,
so rerunning converges.

Use `tambo install` only as shorthand for apply + verify after the plan has
already been reviewed. Use `--skip-install` only when an external package
manager owns installation; it still requires version and functional proof.

For a Creel or `both` plan, `apply` installs the checksum-pinned static browser
bundle, but verification additionally requires browser-owned capability
evidence:

```sh
tambo verify \
  --creel-doctor creel-doctor.json \
  --creel-admission creel-admission.json
```

Those files must come from Creel's doctor and governor interfaces. Never invent
them or translate a human-readable banner into them. Missing evidence, an
unknown required doctor check, stale/unknown policy signals, or a refusal
verdict must stop the install.

`verify-questions` executes every planned `SELECT`/`ASK` through Quipu and
requires its expected marker. A query that runs but answers the wrong shape is
not a pass.

Report the selected profile, each applied version, each verified adapter, and
the retained plan/state paths. Exit status or an “installed” banner alone is
not proof.

When observability is requested, collect explicit `host:port` targets only for
selected tools whose versioned contract declares a metrics endpoint. Run
`tambo render-observability`, then `tambo validate-observability`, and
present all four generated artifacts for review. Rendering is not deployment
and is not evidence that Prometheus scraped a target.

Applied and verified transitions queue automatically under
`.caboodle/episodes`. For work-item snapshots, run `tambo queue-br <export.jsonl>`.
Never put a token in a plan or queue: `flush-episodes` reads only
`QUIPU_AUTH_TOKEN`, requires HTTPS or explicit localhost, and must leave an
identical file pending on any ambiguous response. Report queued and delivered
counts separately.

## Diagnose and resume

- Interview paused at EOF: rerun `tambo init --guided` with the same
  `--session` and `--output` paths if custom paths were used.
- Apply failed before version read-back: keep the state file, fix the named
  installer or PATH problem, and rerun `tambo apply`.
- Apply succeeds with `--skip-install` but normal apply fails: the external
  package manager boundary is the likely cause; state that as inferred until
  its own logs prove it.
- Verify names an adapter: run `tambo verify` again only after addressing
  that adapter. Do not mark the stack healthy because another adapter passed.
- Plan parsing fails: repair or regenerate the plan; never bypass validation by
  editing state.

The CLI has no `upgrade` subcommand. Do not invent one. Update the
`tambo` binary through its documented install path, prove `--version`, then
rerun apply and verify against the existing reviewed plan. A changed tool
version clears its prior verified state until functional verification passes.

## Agent primer

`tambo prime` prints this primer, then each installed stack tool's
`prime --brief`. The copy below is generated from `src/primer.md`.

<!-- BEGIN tambo prime (generated from src/primer.md; do not edit) -->
### Tambo agent primer

#### What Tambo is

Tambo installs and proves the Quipu stack: quipu, camayoc, bobbin, yupana,
desire-path and the stack members (seeds, chaski, shuttle), plus a crew runtime.
It writes a reviewable plan, converges the machine to it, and proves each tool
by version read-back and a functional round trip. It never does a tool's job.

#### Agent workflow

1. Run `tambo prime` to see which stack tools are installed and their state.
2. Run `tambo doctor` before an install; it changes nothing.
3. Write or review a plan (`tambo plan`, `tambo init --guided`) before `tambo apply`.
4. Prove the result with `tambo verify`; trust its output over an exit code.
5. Use `tambo check-updates` before `tambo update`.

#### Core commands

| Command | Use |
| --- | --- |
| `tambo prime` | Print this primer and each installed tool's `prime --brief` |
| `tambo quickstart` | Install the local single-user stack with no interview |
| `tambo doctor` | Report what would stop an install, without changing anything |
| `tambo init --guided` | Run the resumable interview and write a plan |
| `tambo plan --profile <p>` | Write a reviewable plan for a profile |
| `tambo apply` / `tambo verify` | Converge to the plan, then prove each tool works |
| `tambo install` | Apply and verify in one step |
| `tambo check-updates` / `tambo update` | Report and converge drift from reviewed releases |
| `tambo update-self` | Update Tambo from a checksummed release |

Run `tambo help <command>` for the exact syntax of any command.

#### MCP tools

None. Tambo registers the stack's MCP servers (`tambo project-settings`) but serves none.

#### Rikuy site

Rikuy is a static browser brief, with no stack binary to probe. Set `RIKUY_URL`
or pass `tambo prime --rikuy-url https://your-brief.example/` to read only the
site's `source.json`. It runs beside binary probes with a subsecond deadline,
64 KiB response cap and no redirects, retries, credentials or curl config.
The result reports static-site reachability, revision and `generated_at` when
present. Missing generation time stays unknown; it is never the HTTP Date or
check time. This does not measure live brief freshness or activate a reader.
An unconfigured optional site does not degrade the stack summary. A configured
failure keeps its unreachable/unreadable category; only a deadline is partial.
`--static` skips this GET as well as all binary probes.

#### Next steps

- Book: <https://intigraph.github.io/tambo/>
- The stack prime contract: <https://github.com/intigraph/tambo/blob/main/docs/stack/PRIME-CONTRACT.md>
- Skill: <https://github.com/intigraph/tambo/blob/main/skills/tambo/SKILL.md>
<!-- END tambo prime -->
