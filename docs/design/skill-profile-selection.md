# Pinned skill-profile selection

`caboodle::skill_profile::select` validates a selected set of skill releases
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
route. This component writes no configuration, registers no harness entry and
does not expose a skill-sync command. Graph catalogue publication, installation,
retirement and loaded-version history remain follow-up implementation work.

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
