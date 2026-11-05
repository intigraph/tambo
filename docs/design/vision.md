# Tambo — vision and initial plan

> One guided wizard that installs the tool corpus,
> proves each piece actually works, and wires the lot into Prometheus.

## Vision

Tambo is the layer above every tool in the stack. Camayoc ends at knowledge-graph
bootstrapping; tambo begins at "I have a fresh machine and I want this stack."

It is **AI-installable**: an LLM agent (Claude Code, codex, anything that can run
shell) can drive the entire install end to end — and a human gets the same path as a
guided interview. The wizard asks questions: *what do you want (knowledge graph only?
retrieval? a full crew?), where does it run (ports, paths, containers or bare), do you
have Prometheus or should I bring one?* Answers become a **plan file** the user or
agent reads before anything executes.

Three claims separate this from an install script, and each is a proof, not a banner:

1. **Installed** is proven by version read-back, never by exit code.
2. **Working** is proven by a per-tool *functional* round-trip — quipu accepts an
   episode and a control-gated query finds it; bobbin indexes a fixture and a search
   returns it; yupana answers `impact` on a fixture repo. Every check pairs with a
   negative control (the selftest that corrupts a byte and goes red) so a pass is
   capable of failing.
3. **Observable** is proven by Prometheus actually scraping each tool's declared
   exporter — tambo generates the scrape config, a starter alert pack, and a
   consolidated dashboard.

This encodes the operating culture the stack was built under: verify by mechanism,
read back what you wrote, control every zero. Tambo productizes that discipline.

## Architecture

**Convention over manifest.** Each tool is tambo-ready by exposing the house
pattern the repos already follow: conventional CLI verbs (`<tool> selftest`,
`<tool> health`, `<tool> init`) and/or `just` recipes (`just install`,
`just verify`) — tambo discovers and drives these directly instead of parsing a
manifest file. Metrics contracts live as camayoc catalogue entries, not per-repo
config. The engine is a plan → apply → verify loop with a state file recording what
is installed *and last proven working*; idempotent and resumable — re-running
converges, a failed step names itself. The tambo repo itself follows the
sibling-repo patterns: Rust, justfile, the same CI and release conventions.

**The wizard is an interview, not a UI.** `tambo init --guided` runs the question
flow; in agent hands the same flow is a skill. Output is always the plan file —
two-phase (plan, then apply) so nothing installs on the strength of a conversation
alone.

**Profiles**: `kg` (quipu + camayoc), `retrieval` (+ bobbin), `code-intel`
(+ yupana), `crew`, `everything`. The `crew` profile is a choice — **shantytown,
creel, both, or standalone**: shantytown for a tmux-resident crew on a host, creel
for parallel agent bursts entirely in the browser, both together, or standalone —
no crew layer at all, the tools serving a single agent or a human directly. The
shantytown/creel choice rests on their feature parity, tracked as its own workstream.

**Both harnesses are first-class.** Tambo supports **Claude Code and codex**
equally: either can drive the install (the skill and the interview are
harness-agnostic — shell + HTTP only), and when a crew is chosen the wizard asks
which harness each role runs on, emitting the right per-harness configuration.

**Seeds is in the corpus.** The work tracker (`sd`) stores work items in Quipu
and installs like any other tool. Crew profiles use Seeds for work tracking;
Camayoc's purpose-built memory records provide agent memory.

**Tambo emits triples/episodes to quipu.** The engine's state transitions —
installed, verified, upgraded, failed — are emitted as episodes through
camayoc's ingress discipline, so the graph the box just installed immediately
knows what the box contains. Bootstrap order handles the chicken-and-egg:
emissions queue locally until quipu passes its own verify, then flush once.

**Work items are already graph data.** Seeds writes work-item lifecycle changes
to Quipu. Camayoc projects the metadata needed by stack readers through its
ingress discipline. Lifecycle storage and knowledge extraction remain separate:
a work-item update alone does not extract its findings into the ontology.

**Metrics are ontology, not just scrape targets.** Each tool's metrics contract is
also a camayoc catalogue entry, so "what does this series mean and who owns it"
lives in the graph the box just installed.

**Two first-class skills** (portable, shell + HTTP only):

- `skills/tambo` — install, verify, diagnose, upgrade the stack; an agent's
  entry point to the whole box.
- `skills/camayoc` (ships with camayoc; tambo installs it) — bootstrap the
  ontology: interview the user about their domain, derive competency questions,
  load the bootstrap ontology plus the right `.pendant`, run a verified first ingest.

## Initial plan

**Phase 0 — skeleton that proves the idea.**
Repo scaffold; convention adapters for quipu + bobbin (the two with registry
packages); the plan/apply/verify engine; `tambo verify` as a standalone command.
North-star acceptance from day one: **a clean-machine install test in CI** — nothing
that only works on one particular network.

**Phase 1 — the wizard and the ontology skill.**
LLM-guided interview → plan file; `skills/tambo`; camayoc bootstrap-pack
integration. First end-to-end demo: fresh VM → interview → working KG + retrieval
with a verified ingest.

**Phase 2 — observability.**
Metrics contracts for every tool (as camayoc catalogue entries); generated
Prometheus scrape config + starter alerts + one consolidated dashboard. Add yupana
and desire-path adapters.

**Phase 3 — the crew profile and upgrades.**
The shantytown/creel/both choice; `tambo upgrade` re-verifies after; each repo's
CI keeps its conventional verbs tested with releases so the box tracks the tools.

## Per-repo readiness

The cross-cutting contract every corpus repo adopts:

1. The conventional CLI surface — `selftest` / `health` / `init` verbs and/or
   `just` recipes.
2. A `selftest` with a **negative control**.
3. A metrics story: `/metrics` endpoint or textfile exporter, catalogued in camayoc.
4. Versioned release artifacts — install never means build-from-source.
5. `init` writes a starter config; config via file + env with documented precedence.
6. A health probe distinct from the selftest.
7. No environment-specific names in defaults or docs.

Each repo's gaps against this contract are tracked as issues in that repo's own
lane; tambo's adapter for a tool doubles as that tool's readiness checklist.
