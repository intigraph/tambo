# The stack prime contract

Every Quipu stack tool ships `<tool> prime`: the one command an agent runs to
learn what the tool is, how to use it, and what state it is in right now.
`tambo prime` runs every installed tool's `prime --brief` and prints the stack
overview. The reference implementation is `quipu prime`
([intigraph/quipu#892](https://github.com/intigraph/quipu/pull/892)).

## Static primer

One Markdown file in the repo (`src/primer.md` or the language's equivalent),
embedded in the binary at build time. It prints with no network, no store and
no config. Required sections, in this order:

| Section | Holds |
| --- | --- |
| `# <Tool> agent primer` | The title |
| `## What <tool> is` | Three to six lines: the job, and where it sits in the stack |
| `## Agent workflow` | Numbered steps; query-first where the tool reads shared state |
| `## Core commands` | Table: command, one-line use |
| `## MCP tools` | Table: tool, one-line use; "None" when the tool serves no MCP |
| `## Next steps` | Links: book or README section, skill, man page |

Tools add sections between `Core commands` and `Next steps` (quipu adds
`Writing safely`). `--section <name>` prints one section, matched by a
case-insensitive heading prefix; an unknown name exits 2 and lists the sections.

## Live summary

After the primer, `prime` prints `## Live state`: a short summary of the tool's
own state (bobbin: index freshness; seeds: your plate; st: crew and your
anchor; chaski: subscriptions; yupana: base ref and tiers).

- Bounded: the summary finishes in under 1 s. Enforce a deadline inside the
  tool, cap every list, and report `partial` when the deadline cuts it short.
- Read-only: open stores read-only, never create a missing file or directory,
  never take a write lock, never call a mutating endpoint.
- Offline-safe: a missing store, a down server or no network still prints the
  full static primer and exits 0.
- Plain wording: say `unreachable` (cannot connect), `unreadable` (found it,
  cannot read it) or `absent` (nothing there), name the path or URL, and give
  the one command that fixes it.
- `--static` skips the live summary.

## `--brief`

At most 10 lines, at least 5: line 1 is `<tool> <version>: <one-line job>`,
then the live summary condensed to its key lines, then the line
``Run `<tool> prime` for the full primer.`` This is what `tambo prime` shows,
and it truncates anything longer.

## `--json`

Plain text is the default. `--json` prints one object with this shape, and
`--brief --json` drops `primer.text`:

```json
{
  "tool": "quipu",
  "version": "0.42.0",
  "primer": { "sections": ["What Quipu is", "Agent workflow"], "text": "# Quipu agent primer\n..." },
  "live": { "status": "ok", "elapsed_ms": 12, "summary": { "last_tx": 4512 } },
  "status": "ok"
}
```

- `live.status`: `ok`, `partial`, `absent`, `unreachable`, `unreadable` or
  `skipped` (`--static`). `live.summary` holds the tool's own fields.
- `status`: `ok` when the live summary is `ok` or `skipped`, else `degraded`.
  `prime` exits 0 in both cases; a bad flag exits 2.

## Single source, generated copies

- The skill (`skills/<tool>/SKILL.md`) carries the primer between
  `<!-- BEGIN <tool> prime` and `<!-- END <tool> prime -->` markers, generated
  from the source file.
- The docs page includes the source file (`{{#include}}` in mdBook) or is
  generated from it. Never edit a copy by hand.
- A test in the normal CI run fails when a copy drifts, and says the one
  command that regenerates it (quipu: `QUIPU_BLESS_GOLDENS=1 cargo test --test prime_sync`).

## `tambo prime`

`tambo prime` prints Tambo's own primer, then, for each tool in the
[stack table](https://github.com/intigraph/tambo/blob/main/docs/stack-table.md) found on `PATH`, the output of
`<tool> prime --brief` (2 s timeout, run in parallel). A tool without a
working `prime` is listed as `no prime yet`; a tool past its timeout as
`timed out`; neither fails the command.
