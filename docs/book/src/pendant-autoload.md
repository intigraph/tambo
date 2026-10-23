# Design: pendant autoload, monorepo shares and graph pointers

**Status:** repository sync and opt-in hooks are implemented in source.
Monorepo derived shares and unloaded graph pointers remain proposals. Source
implementation is separate from release, installation and opt-in activation.

Once repositories routinely ship a quipu pack (`.pendant.tar.gz`), a fresh clone
should have that knowledge in the local graph without anyone running a command.
This document covers three questions: how a clone discovers and loads its
packs, how a monorepo ships one pack per project, and how a pack can point at
graphs that are not loaded yet.

## What exists today

- **Packs.** A `.pendant.tar.gz` is a deterministic text bundle, not a database
  file. Its manifest carries `parent_share`, `tx_anchor`, `graph_hash`,
  `shapes_hash` and a stored CONSTRUCT scope.
- **Composition.** `quipu compose` loads verified packs into one store. Each
  pack keeps its own named graph, and their union is named as a dataset.
  Exact IRIs join across packs; blank nodes stay scoped to their pack.
  Different shape bundles need an explicit `--shapes-from`. A nonconforming
  union stays in quarantine, and nothing is promoted to ROOT. A local
  retraction survives a reload of the same snapshot. See quipu's
  [pack composition](https://intigraph.github.io/quipu/concepts/pack-composition.html).
- **Manual loading.** `install-stack.sh --pendant PATH` loads a pack by hand, and
  `scripts/setup-environment.sh` stages and verifies packs in cloud
  environments.
- **Federation.** quipu reaches remote graphs through SPARQL `SERVICE` and
  federated query fan-out, but only endpoints declared in the federation
  configuration.

Repository discovery is available through the explicit sync command below.
Nothing installs or activates repository hooks without an opt-in.

## 1. Discovery and autoload

**Convention, not a manifest.** Tambo does not parse per-repo manifest
files ([convention over manifest](conventions.md)), and packs need none. A
repository ships its packs at one conventional path:

```text
.quipu/packs/*.pendant.tar.gz
```

Each pack already describes itself: its own manifest, inside the archive,
carries the graph IRI, `graph_hash`, `shapes_hash`, `parent_share` and scope.
Discovery is a directory listing, and nothing in the repository can drift out
of step with the packs it describes.

**Triggers.** A `post-checkout` and `post-merge` hook, installed by
`tambo init --pack-hooks` in repositories the user opts in, runs the same
sync command (the `tambo` compatibility executable supports it too):

```bash
tambo packs sync --repo . --shapes-from /path/to/trusted-authority.pendant.tar.gz
```

The command fingerprints all discovered archives. Trust is checked on every
run, including a replay. When any archive changes or disappears it composes the
whole remaining set, not just the changed file. Quipu moves the repository's
CURRENT pointer only on a successful composition. An unchanged receipt skips
composition only while its authority, repository identity, database and WAL
stamps still match. A missing database triggers a fresh composition. Receipt
replay is a local cache check, not a fresh query proving every stored fact.

A trusted authority pendant is required when there are packs to load. Supply
one exported from the operator's reviewed store; it is not selected from the
repository's data or used to install constraints. Its shapes govern the union,
and its own facts also participate in the composed dataset, so use an empty-fact
authority pendant when only its shapes should be included. Every repository
archive is copied to private temporary storage before hashing and composing;
Quipu verifies those exact frozen bytes. Sync accepts at most 64 archives and
256 MiB of total archive bytes, including the authority. A refusal or quarantine
never becomes a successful-load receipt, and no automatic retry is attempted.

By default the database and receipt live under the Git directory's
`caboodle-pendants/`; `--db` selects a different local database. Repository
identity comes from `origin` (`host/owner/name`), and pack identity is its stable
file name. No per-repository manifest is needed. Only `.pendant.tar.gz` files are
discovered; old `.qpack` names must be migrated deliberately. Symlinked discovery
directories and archives are refused.

Opt in to both Git hooks with:

```bash
tambo init --pack-hooks --repo . --shapes-from /path/to/trusted-authority.pendant.tar.gz
```

Existing hooks are retained, never appended to or replaced. If `core.hooksPath`
is configured, have its owner call the sync command there instead. The generated
hooks use the chosen executable and authority path. Hook installation does not
compose immediately; a later checkout or merge runs sync. Git hooks can be
shared by worktrees of one repository. Remove only these generated hooks to opt
out; ordinary guided initialization does not install them.

Cloud setup uses the same command when `CABOODLE_PACKS_REPO` names an available
checkout and `CABOODLE_PACKS_SHAPES_FROM` names its trusted authority. If setup
runs before checkout, it explicitly reports that sync did not run: invoke sync
after checkout rather than treating the environment cache as an autoload proof.
Optional setup failure is reported but still leaves the session's exit status 0.

**Stable names and rollback.** Quipu's delivered repository mode provides
`urn:quipu:repo:<host/owner/name>:current` and `:previous`, retains both snapshots
and carries local retractions across snapshots. Query these dataset aliases with
SPARQL `FROM`, not `GRAPH`. `quipu compose --repo host/owner/name --rollback
--db <local-db>` swaps the pointers. Sync does not promote anything to ROOT;
rollback and broader consumer installation remain explicit operator actions.

## 2. Trust

A pack is data landing in the graph that agents query first, so loading is a
trust decision.

**Exists today** (aegis-2eckx0): `tambo packs trust` keeps these decisions in
`~/.config/caboodle/pack-trust.toml`, separate from the stack config.
`owners` lists `host/owner` pairs whose packs autoload. `[[allow]]` admits one
foreign archive and must name both `repo` and `pack_sha256`: an entry without
the hash, an unknown key or a malformed value refuses the whole file, so every
pack is refused until it is fixed. `[[allow_endpoint]]` admits one exact
`http(s)` endpoint with no credentials in its URL. `tambo packs trust check
<remote> <sha256> --pack <path>` exits 0 to load, or 3 with a refusal naming the
pack and the exact `allow` command. `packs sync` asks this question before composing and before accepting a
cached replay.

`packs trust check-endpoint <remote> <url>` asks the same policy whether a
pointer endpoint may join federation: exit 0 admits it, exit 3 refuses it.
It reads the current trust file on every invocation and never changes it.
An archive allow does not admit its endpoint; foreign endpoints need their own
exact repository/URL allow. Both `tambo` and `tambo` expose this command.

- **Your own repositories load automatically**, meaning repositories whose
  remote owner matches an allow-list in Tambo's configuration.
- **A foreign pack needs an explicit allow**, recorded per repository and pack
  hash. A repository name appearing in a configuration file is not consent.
- **Loading is not promotion.** An autoloaded pack stays in its named dataset,
  queryable through the current pointer. It never enters ROOT. Promotion
  remains a separate, reviewed step, as composition already enforces.
- **Shape authority stays with the store.** Sync requires an operator-controlled authority
  pendant (`--shapes-from`), supplied independently of repository discovery.
  Repository packs cannot choose that authority or install constraints.

## 3. Monorepos

A monorepo ships **one parent share and one derived share per project**.

- The parent share covers the whole repository. Each project is a derived share
  (`parent_share` set) whose stored CONSTRUCT scope filters by path prefix,
  such as `packages/api/`.
- Each derived pack's own manifest records its path scope, so
  `tambo packs sync` can reload only the projects whose paths changed in
  the checkout, the same idea as yupana's per-file keys, grouped by project.
- **Cross-project edges survive a reload.** Entity IRIs do not depend on the
  commit, so an edge from one project to another still resolves after either
  side reloads.
- **Sparse checkouts** load only the projects present. The projects that are
  absent can be reached through a pointer (section 4) instead.
- **Ownership:** a CODEOWNERS entry becomes `owned_by` on the project's
  entities.
- **Recommendation:** no combined all-projects pack at first. Composition of
  the derived shares already gives the union. Add a combined pack only if cold
  start is measured to be too slow.

Test case: `scbrown/reckoning`, a real pnpm monorepo.

The monorepo implementation adds an explicit `packs sync --monorepo` mode. It
reads native share manifests, finds the one parent referenced by the derived
shares, and excludes that parent from CURRENT. Each child uses the canonical
CONSTRUCT path scope printed by `packs project-scope`. A child needs a visible tracked file whose index entry is not marked skip-worktree, under a directory without symlink redirects. An excluded directory left behind by untracked files does not count as a checked-out project; those files are preserved. Keep
`.quipu/packs/` in a sparse checkout. Unchanged members reuse Quipu's committed
pack graphs; changing one share creates only that member's new graph. Selection
is part of the version-2 monorepo receipt, so changing sparse membership reconciles CURRENT. Flat mode retains the baseline version-1 receipt format. Downgrading a monorepo consumer requires its retained pre-upgrade receipt and database rollback; an older binary refuses version-2 metadata rather than silently loading all projects.
The parent may itself name a previous snapshot. All archives, including excluded
ones, are trust-checked and verified by native Quipu in an isolated temporary
store before a changed composition is accepted.

For a producer with an existing graph and loaded shapes, generate the scope,
then use native `quipu share` with the parent share's identity:

```sh
tambo packs project-scope --graph urn:source:repo --path-predicate urn:source:path \
  --prefix packages/client/ > client.construct
quipu share --construct "$(cat client.construct)" --parent-share "$parent_share" \
  --shapes project-schema --no-queries --db producer.db --output client-share
```

Archive the parent and each child as separate `.pendant.tar.gz` files under
`.quipu/packs/`. No all-projects union pack or repository manifest is required.
The native manifest binds both the query and `parent_share` into share identity.
Scope readers deliberately refuse arbitrary query variants rather than guessing
which paths they cover. Regenerate a scope with `packs project-scope`.

To include CODEOWNERS-derived ownership, add `--repo . --db producer.db
--ownership-predicate urn:source:owned_by` to `packs project-scope`. Supply the
existing schema's predicates; these URNs are examples. The command reads the
first CODEOWNERS in `.github/`, the repository root, or `docs/`, applies the last
matching rule to tracked project files, and includes ownership in the native
CONSTRUCT. It supports GitHub users, teams and email owners. Empty owner rules
clear earlier matches. Unsupported patterns (`!`, bracket ranges and backslash
escapes), invalid identities, symlinked CODEOWNERS, and bounded input overflows
refuse. When CODEOWNERS is absent it invents no owners. Git's matcher runs in a
private repository, with no global ignore file or case folding.

Owner resources must already exist in the producer graph. Current native
CONSTRUCT export can otherwise turn an unknown constant IRI into a literal;
scope generation checks each resource and refuses unknown or unreadable owners.
Register resources through the producer's governed ingest path before exporting.
Generation reads the producer; it does not register owners or change ROOT.

Opt-in hooks retain this mode with `init --pack-hooks --monorepo`. Cloud setup
uses `CABOODLE_PACKS_MONOREPO=1` alongside the existing repository and authority
settings. Neither mode is armed by default. See the replayable
Reckoning demonstration in `docs/demos/monorepo-sync/`
for the pinned checkout, explicit synthetic CODEOWNERS and isolated stores.


## 4. Pointers to graphs that are not loaded

A pack's own manifest can name a graph the pack does not contain, so pointers
need no repository-level file either. A repository that ships only pointers
ships a pack with no facts.

```toml
[[pointer]]
graph = "https://example.org/homelab"
kind = "endpoint"                     # or "pack"
endpoint = "https://quipu.example.org/sparql"
```

- **`kind = "endpoint"`**: a live, large or fast-moving graph, such as a
  homelab operations graph. `tambo packs sync` adds it to quipu's declared
  federation list, so `SERVICE` and federated fan-out reach it with no local
  copy. The trust rule of section 2 applies to the endpoint: declaring it in a
  foreign pack does not authorise it.
- **`kind = "pack"`** (with `url` and `sha256`): a pinned, offline-capable
  pack. It is fetched, verified and composed on first use rather than at
  checkout. This also works in the browser build.

A sparse monorepo checkout can federate to the projects it did not check out
this way.

## Test cases

| Case | Covers |
| --- | --- |
| quipu's own repository plus a homelab operations pack | sections 1, 2 and 4 |
| `scbrown/reckoning` (pnpm monorepo) | section 3 |
| A pack with a tampered archive | the hash refusal |
| A foreign repository with no allow entry | the trust refusal |
| Two commits of one pack | pointer moves; previous kept; retraction carried |

## Proposed children

1. **quipu:** the per-repository current pointer and snapshot retention, with
   retraction carry-forward across snapshots of one pack.
2. **caboodle:** the `.quipu/packs/` convention, `tambo packs sync`, and
   the opt-in git hooks.
3. **caboodle:** the trust allow-list for foreign packs and pointer endpoints.
4. **quipu and caboodle:** monorepo derived shares by path prefix, proven on
   `scbrown/reckoning`.
5. **quipu:** pointer entries in the pack manifest, and lazy `kind = "pack"`
   pointers.
