# Choose tools for your use case

Start with a problem, then add tools when you need them. These are suggested
workflows, not additional Tambo profiles or a promise that every connection is
configured for you. Tambo installs and verifies a selected set; indexing,
agent registration, event sources and policies still need their own setup.

## A developer working with a coding assistant

**Goal:** find the right code and carry useful context between tasks.

**Start with:** Tambo's `retrieval` profile: Quipu for durable knowledge,
Camayoc for checked knowledge ingress and Bobbin for code search/context.
`tambo quickstart` is the local, single-user route. Index the repository you
want to work on and register its server as described in the agent guide.

**Example workflow:** search for the code behind a behavior, make a change,
then preserve its rationale and source as knowledge for the next task.

**Add when needed:** Yupana through `code-intel` for callers and change impact;
Desire Path to capture failed tool calls rather than repeating them. Policy
checking needs configured policies; an index alone is not enforcement.

Next: [getting started](getting-started.md), [using agents](agents.md),
[profiles](profiles.md).

## An engineering team or agent-crew lead

**Goal:** keep ownership, progress and handoffs visible across several workers.

**Start with:** Seeds for work items and Shantytown for a host-based agent crew.
The `crew` profile includes the knowledge/retrieval tools and the Chaski,
Seeds and Shuttle members. Choose the runtime explicitly, for example
`tambo plan --profile crew --crew shantytown`, then review the plan before apply.

**Example workflow:** assign a tracked task, let an agent work on it, record the
result with provenance and use a signed workflow for a handoff that needs an
explicit transition. Configure Chaski reactions for the events you want delivered.

**Add when needed:** Rikuy for a shared live brief; Yupana for code-policy and
impact checks. A live brief needs its data sources, and notifications need
configured subscriptions. Neither follows automatically from installing tools.
The optional `creel`, `both` and `standalone` runtime choices are described in
profiles; `both` retains explicit handoff rather than automatic fan-out.

Next: [profiles](profiles.md), [crew contracts](crew-contracts.md),
[the stack](stack.md).

## A knowledge curator or researcher

**Goal:** retain evidence and vocabulary so a claim can be traced and revisited.

**Start with:** the `kg` profile, Quipu plus Camayoc. Quipu keeps provenance and
history; Camayoc prepares and checks ingress. Quechua supplies the shared
vocabulary and shapes that help tools agree on the meaning of facts.

**Example workflow:** prepare a source-backed episode, validate its terms and
provenance, admit it through the configured gate, then query the recorded facts
and their history. Choose the write rules you need rather than assuming every
installation refuses the same inputs.

**Add when needed:** reviewed corpus shares, or Chaski to notify you when relevant
knowledge changes. Shares are staged imports; Tambo does not automatically promote
them into ROOT. Repository-pendant availability is a separate publication gate.

Next: [the corpus](corpus.md), [profiles](profiles.md),
[Quipu's book](https://intigraph.github.io/quipu/).

## A platform or service operator

**Goal:** connect service changes, delivery records and operational decisions.

**Start with:** Quipu and Camayoc for source-backed operational knowledge and
delivery ingress. Tambo supplies a reviewable plan and verification; start small
with `kg` and review additional members as your workflow grows.

**Example workflow:** configure a supported delivery emitter, record a deployment
outcome, query its change history and use that evidence in an operational decision.

**Add when needed:** Chaski for event-driven notifications, Shuttle for signed
runbooks and Rikuy for status and decisions in a live brief. Configure each event
source, subscription and view explicitly. Tambo does not enable deployment
emitters or prove DORA coverage; missing history is not a measured zero.

The `everything` profile is a broader tool selection, not an all-services
production deployment. Check the generated plan and each tool's proof boundary.

Next: [the stack and its proofs](stack.md), [profiles](profiles.md),
[Camayoc's delivery catalog](https://github.com/intigraph/camayoc/blob/main/catalog/README.md).

## An individual organizing projects and decisions

**Goal:** keep tasks and the reasons behind decisions together.

**Start with:** Quipu and Camayoc for durable notes with provenance; add Seeds
when the work needs tracked items. A `crew` plan with `--crew standalone` makes
the tools available without installing a crew runtime.

**Example workflow:** record a decision and its source, create the related work
item, then update its state as the work progresses. Query the evidence again
when you revisit the decision.

**Add when needed:** Chaski for configured reminders or notifications, and Rikuy
for a live brief. This is a knowledge-and-work workflow, not a built-in calendar
or personal-planning profile. No agent crew is required.

Next: [profiles](profiles.md), [using Tambo](using.md), [the stack](stack.md).

## Turn a recommendation into an installation

The available profile names are `kg`, `retrieval`, `code-intel`, `crew` and
`everything`. The canonical [stack table](stack.md#related-projects) describes
the twelve projects; it is not an install manifest. In particular, `everything`
does not mean all twelve projects are installed. Inspect the plan's actual
members, review [compatibility](compatibility.md), and retain the plan and
state paths so you can resume or verify the same installation.
