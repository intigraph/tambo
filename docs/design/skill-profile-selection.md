# Pinned skill-profile selection

`tambo::skill_profile::select` validates a selected set of skill releases
before the installer performs any I/O. Each pin names a skill, an exact native
pendant `share_id` and a version. Selection uses all three values and produces a
stable order for Claude and Codex consumers. Native share identifiers retain
their `sha256:` algorithm tag and 64 lowercase hexadecimal digits.

The caller supplies a complete, transaction-pinned catalogue result. Missing
pins, duplicate identities, conflicting destination names, unsupported harnesses
and malformed metadata refuse. A public-only profile refuses internal releases.
An empty result cannot become an instruction to remove installed skills.

This module is an offline source component. The graph adapter still needs to
establish the profile's governed type, explicit dataset, transaction and query
coverage. Native pendant import/composition must verify the selected bytes and
their authority. The renderer must enforce file ownership, hashes and atomic
publication. A successful selection alone establishes none of these proofs.

The live MCP/skill tooling manifest keeps its existing owner and projection
route. The selector itself writes nothing. The explicit local publisher and
whole-profile staging adapter below expose `tambo skills publish` and
`tambo skills sync`; neither changes that manifest or registers a harness.
Authenticated catalogue transport, scheduled host adoption and graph-backed
loaded-version history remain separate integration steps.

The synthetic controls compose three pins for both harnesses, verify stable
ordering, and exercise partial results, stale or wrong pins, ambiguous releases,
scope boundaries and unknown input fields.

Run the offline demonstration with `cargo run --example skill_profile`. It
prints the three selected releases and observes refusal after substituting an
unavailable hash. The hashes and identifiers are synthetic fixtures; the output
does not certify a published pendant or a loaded harness.

## Native query bridge

`skill_profile::query::plan` constructs one native Quipu request for an explicit
profile IRI, graph IRI and positive source-local transaction. It uses the native
`tx` field and verbose IRIs; it never combines unsupported read clocks or silently
selects current state. No query is executed by the library. The caller must bind
the response to that exact request and establish the server and shape authority.

The query retains all properties of selected pins rather than inner-joining away
malformed records. The decoder requires explicit record types, RDF term kinds
and datatypes, complete columns, matching row count and `truncated: false`. A
4 MiB byte budget and 8,192-row budget refuse oversized responses; the query
requests one extra row as a saturation sentinel. Missing metadata and a partial
profile refuse instead of becoming a smaller installation. Scope and harness
checks run through the same pinned selector.

The offline example prints a request without transport:

```sh
cargo run --example skill_catalogue_query -- plan urn:profile urn:catalogue 1
```

With a response obtained through that exact request, `decode` accepts a final
response-file argument. The checked-in synthetic response fixture was produced
by the native Quipu 0.15.0 query tool at source revision `c61eed92`, against a
private 55-fact store: three releases selected for both harnesses; selecting an
empty graph refused. It is a wire-format control, not a live catalogue receipt.
CLI `query` text tables are not this JSON contract. No credential handling,
server selection, graph writes, retirement or harness installation is added.

## Compile a skill source tree

`python3 scripts/skill-render-source.py skills/example` emits native RenderSpec
data on stdout. It writes no graph, share archive, installation or settings.
The owning repository remains the authoring source; the result is input to the
existing governed native share builder, not a sealed pendant or trust receipt.

The compiler requires an explicit regular directory and a UTF-8 `SKILL.md`.
It reads through no-follow directory/file descriptors and refuses symlinks,
special files, hidden or template-active paths, concurrent metadata changes,
unsupported modes and oversized trees. It streams at most 1,024 total entries before sorting (including directories),
and accepts at most 512 files and 8 MiB;
`SKILL.md` is at most 256 KiB with mode 0644. Supporting files retain 0644/0755
modes and carry native `sha256:` hashes plus base64 bytes.

Skill markdown is query data, never template source: literal template syntax
and front matter remain unchanged. Native rendering revalidates include hashes
and enforces the existing destination ownership and atomic publication rules.
Compilation does not decide public scope or scan for private information; the
existing source review and share scrub remain required before publication.
No catalogue identity, release version or share ID is invented by this step.


## Native publisher and whole-profile staging

On Unix, `tambo skills publish` compiles one explicit source tree, validates a
native RenderSpec in a disposable database, builds a native text pendant and
verifies it through native import. It emits a compressed native share,
`release.json`, the original metadata and `catalogue.ttl` in a **new** output
directory. Existing output refuses. `release.json` is the last completion
marker; an I/O failure can leave a partial directory that must be inspected,
not overwritten by a retry. Import quarantine stays quarantine. Integrity
verification is not permission to promote a share or publish a catalogue.

Metadata is JSON with `skill`, `name`, `description`, `repository`,
`source_path`, `version`, `render_spec`, `public` and `harnesses`. Identifiers
are absolute IRIs; `source_path` is provenance, never a path to fetch. The
existing selector validates destination names, pins and supported harnesses.
The native share ID comes from the actual native manifest, never from an
archive filename or a guessed content hash. The source stays in its owning
repository; no source repository URL is fetched by this adapter.

```sh
tambo skills publish --source skills/example --metadata release-metadata.json \
  --render-shapes reviewed-render.ttl --output new-release --quipu /owned/quipu
```

Internal publications use the native internal destination. A public release
requires the native outward identifier-policy gate; supply the reviewed
`--identifier-policy` and `--identifier-shapes` together. The same native
outward check covers generated catalogue metadata after the share ID is known.
The catalogue cannot embed its own share ID inside the hashed share it names.
No shared graph registration or release workflow is installed by publication.

`skills sync` takes a `SkillProfile` JSON file, a `CatalogueSnapshot` JSON file,
a local JSON map from each native share ID to its compressed share path, and
an explicit caller-selected native shape-authority share. It calls
`skill_profile::select` **before** opening archives or touching the destination.
It refuses missing/duplicate pins, incomplete metadata, wrong versions,
unsupported harnesses and public-only/internal conflicts. These files are
caller-supplied data, not authenticated server or graph authority. A graph
adapter must establish the explicit dataset, transaction and complete response
before constructing them; this command neither fabricates that evidence nor
silently substitutes current server state.

```sh
tambo skills sync --profile profile.json --catalogue catalogue.json \
  --bundles local-bundles.json --authority reviewed-authority.pendant.tar.gz \
  --render-shapes reviewed-render.ttl --dest owned-profile --quipu /owned/quipu
```

The adapter freezes bounded regular archive and shape files, checks each
manifest's native identity against its selected pin, and invokes native
composition with the separate caller-selected authority. Native composition
verifies the complete frozen shares and decides admission. A public-only
profile also requests native outward policy; metadata cannot relabel an
internal share as public. No repository, source-path or render-specification
IRI becomes a URL to download. Compressed and decompressed archives have
aggregate 256 MiB budgets; profile includes are capped at 512 files and
16 MiB of base64 data. Native per-operation limits still apply.

Each selected specification is rendered privately, preserving raw markdown,
binary bytes and executable modes, including any source-provided
`agents/openai.yaml`. The source compiler rechecks paths and
file limits. One aggregate native RenderSpec then publishes the complete
`claude/<name>/` and `codex/<name>/` trees plus `profile.json` into the explicit
owned destination. Native rendering owns publication, NOOP detection and
retirement of previously owned files. Unknown or changed destination entries
refuse; the adapter does not delete or replace a shared harness directory.
The same selection produces a NOOP without changing bytes, inode or mtime.

`profile.json` records the selected pins and source transaction. It is a
staging receipt, **not proof that a harness loaded them**, and is not a
server-authenticated history record. Both command reports explicitly retain
`harness_registration: false` or `catalogue_registered: false`. Normal settings
owners, scheduled lanes and fresh-session proof remain required for live use.
No host configuration, symlink, scheduler or shared graph is changed here.

The explicit private integration control publishes two public skills plus one
private skill, composes them for both harnesses, checks exact bytes/modes and
NOOP identity, and retains the destination across incomplete/wrong-pin/scope,
corrupt-share and foreign-file refusals. It then retires only an owned skill:

```sh
python3 tests/test_skills_native.py --tambo /owned/candidate-tambo \
  --quipu /owned/quipu --render-shapes /reviewed/render.ttl
```

The caller supplies the actual binaries; the test never downloads, installs or
opens a served database. Standard unit tests also cover early refusal before
native execution and no-follow input reads. Linux fixture results are not a
Mac deployment or a fresh Claude/Codex consumer census.
