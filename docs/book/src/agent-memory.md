# Agent memory setup

On Unix, `tambo memory` stages a reviewable memory setup for Claude Code and
Codex managed by shantytown. It uses Bobbin's existing prompt hook for recall,
Camayoc's generated skills and capture commands, and shantytown's native Stop
ruling check. Install current, reviewed `tambo`, `st`, `cy` and `bobbin` binaries
first through their normal installation paths. Memory apply configures those
tools; it does not install or upgrade them.

## Review, apply and restore

Choose the actual shantytown root, Bobbin configuration and person entity.
The Camayoc checkout supplies skills and the MCP tool catalog from its committed
HEAD, whose full revision is recorded in the plan. Uncommitted source changes
are excluded. Optional `--claude-home` and `--codex-home` select the intended
harness homes; otherwise the usual home directories are used.

```sh
tambo memory plan \
  --root /path/to/shantytown \
  --bobbin-config /path/to/bobbin/config.toml \
  --person http://example.org/people/operator \
  --redirect-mode copy \
  --camayoc-source /path/to/camayoc \
  --output /private/path/memory-plan
```

Review `plan.json` and the staged bundle files locally before applying. The
private plan contains original configuration bytes for restoration and may
therefore contain credentials. Keep it outside repositories and shared logs.
`--redirect-mode copy|block|off` records the redirect choice. The default `copy`
preserves memory-file writes. Choose `block` explicitly to refuse those writes,
or `off` to omit the redirect hook and its firing requirement. An existing
standalone redirect installed by `scripts/memory-redirect-hook.py` must be
uninstalled before planning or applying this setup; Tambo refuses duplicates.

`--issuer-name` optionally supplies the person's display alias for capture.

```sh
tambo memory apply --plan /private/path/memory-plan
tambo memory verify --plan /private/path/memory-plan
tambo memory restore --plan /private/path/memory-plan
```

Apply checks required CLI capabilities and compares the installed Camayoc MCP
server's read-only `initialize` and `tools/list` responses with the producer's
catalog. It never invokes an MCP write tool as a probe. It installs the generated
Claude and Codex skills, enables the capture and native Stop flags, and registers
bundles through `st ops hooks`. Shantytown reprovisions its own harness settings;
Tambo does not edit emitted settings or invent an MCP server registration.
Canonical MCP registration remains the settings owner's responsibility.

Bobbin retains an existing positive knowledge budget; an absent budget defaults
to 15 percent. A zero or invalid budget refuses the plan. Recall remains in
Bobbin's prompt hook, with no second retrieval hook.

Apply can resume after an interrupted operation. Restore accepts only the
original or planned bytes and bundles, preserves intervening user edits by
refusing them, and restores the exact prior bundles including evidence fields.
Empty memory
skill directories created by apply are removed; preexisting directories and
user additions are preserved.
Keep the private plan until restoration is no longer needed. Restoring requires
the settings owner to reprovision successfully; inspect failures before retrying.

## Prove the running setup

Reload normal Claude and Codex sessions after applying. Exercise the prompt,
redirect and Stop paths in each harness. Verification requires the settings
owner to report configuration, a live carrier and recent firing evidence for
each required event in both harnesses. An idle role is reported separately and
does not require every fleet member to restart. Configuration alone cannot pass.

The memory wrapper delegates to Camayoc and records only plan, harness, event,
session, completion time and exit code. Ordinary hook failures exit zero; an
intentional memory redirect may return two. Claude has a SessionEnd event.
Codex delegates bounded session-end capture from Stop because its owner does
not support SessionEnd. SessionStart attempts offline replay through `cy flush
--hook`. A receipt from an earlier apply or a restored plan is insufficient.
Manually invoking a wrapper is not evidence of normal harness integration.

`hook_setup_status: verified` establishes the hook setup checked by this command.
The overall report remains `pending` and `auto_memory_eligible` remains false
until independent rollout evidence establishes:

- migration completion and conflict clearance;
- at least 18 of 20 recall prompts without superseded hits;
- the native Stop ruling reminder;
- typed promotion within ten minutes for both harnesses;
- fresh SessionStart Handoff and Directive pins.

The current `disable-auto-memory` command refuses while those gates lack an
accepted proof path. It never edits the user's auto-memory setting, including
an explicit existing true or false value. Hook firing does not establish recall
quality, migration completeness or successful typed promotion.

## Learnings

The Cognee study informed reversible rollout, memory-file redirection, bounded
capture, promotion and offline replay. Existing Bobbin prompt recall remains the
retrieval surface. This setup does not copy a Claude observer, capture every
Read or Grep, or send whole session histories as recall queries. Producer-owned
schemas and skills reduce drift between setup instructions and create tools.
See Camayoc's `docs/design/agent-memory-entities.md` for the entity and rollout
contract, and `docs/design/memory-capture.md` for capture behavior.
