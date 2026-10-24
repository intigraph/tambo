# Shared stack terminal recorder

`terminal-harness/render.py` is the canonical, standard-library Python recorder.
Other stack repositories can vendor these exact bytes and record the origin
commit and SHA256 in their demo guide. It needs no service, browser, model or
network to render. A recipe's commands may have their own prerequisites.

The fixed theme is 24px monospace on an opaque dark terminal. Every complete
command/output page is checked for private paths, internal hostnames, addresses,
credentials, work-item identifiers and unsupported terminal controls. A refusal
publishes no new artifacts; older artifacts, if any, remain unchanged. This is
a conservative leak check, not a substitute for reviewing every visible frame.

A JSON tape can set `version_args` per tool (for example, `{"dp": ["version"]}`);
the default version probe is `--version`. It declares argument arrays, stdin, expected
exit, required output and deliberate reading holds. No shell is involved.
Stderr is refused unless explicitly allowed and is then included in the recording;
stdout precedes stderr in the transcript, rather than claiming interleaved timing.
Long output wraps and pages without omission. The asciinema capture clears the
screen at page boundaries, matching the SVG. Holds total 10–30 seconds; measured
execution times are separate in the manifest. The animation's final frame is
also its fallback when SVG animation is disabled. Adjacent plain transcripts
provide an accessible static alternative.

The caller must prepare a private workspace containing the explicit
`.terminal-demo-sandbox` marker. The recorder uses its `.demo-home` and an
allowlisted environment, never the caller's endpoints, credentials or config.
`BOBBIN_GPU=0` forces CPU mode even before a Bobbin fixture config exists.
Commands can write to that sandbox. Review recipes before running; this helper
is not an operating-system sandbox. It never installs tools itself.

Invoke it with a tape, `--workspace`, one `--tool NAME=PATH` for each executable,
and `--output`. Paths to stdin/manifest inputs must stay within the workspace.
The manifest records native versions, executable and input digests, exact output,
execution time and artifact digests. Real execution time may vary, while the
explicit reading timeline stays stable. No external fonts, images or scripts
are embedded in the SVG. No CI workflow or README integration is introduced.

The existing `pytest tests -q` CI lane collects
`tests/test_terminal_harness.py`. Each control prints its node ID so the hosted
log proves it ran even in quiet mode. No separate workflow is required.
