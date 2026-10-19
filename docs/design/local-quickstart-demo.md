# Local quickstart

Trigger: a new user wants the local stack, with no issued token or running
Quipu server. At base `70199b472793ff94e1639c3d9c1b405ac1b3396d`,
`engine::verify` unconditionally required a canonical credential and a live
server; the README required administrator provisioning. The embedded adapter
already supported a local write/read round trip.

The candidate adds `tambo quickstart`, retains the resulting retrieval plan
and embedded `.caboodle/quipu.db`, and makes remote verification opt-in through
`QUIPU_SERVER` or `quipu_mcp`. `tambo verify` repeats the functional checks.
Quickstart reuses an existing plan without overwriting it. No interview or
local signing-key setup is required to install and verify.

Reproduce from this checkout:

```sh
bash scripts/test-readme.sh
cargo test --locked --test cli quickstart_local
cargo test --locked --test cli remote_mcp_plan_requires_credential
cargo test --locked --test cli live_credential_failure
```

Observed output from the real fresh-HOME README proof:

```text
README local quickstart + verify: passed in a fresh HOME without issued token or server
```

This is a transcript, not a recording or a release install-time measurement.
The proof builds a release-shaped candidate archive, extracts the actual
README shell commands, installs the real pinned tools in an empty HOME, and
requires two `verified` lines for each of Quipu, Camayoc and Bobbin. It asserts
that no canonical token was created and no credential gate ran. Camayoc manages
its own temporary loopback verification server. CLI fixture tests separately
retain the missing-token, bad-token, permissions, unauthenticated-open-server,
and explicit remote-plan refusal controls. These fixtures are not real remote
service acceptance.

The official release and a new timed clean-room run remain separate delivery
steps. No publication or remote trust enrollment is performed by this change.

A paired CLI-fixture probe was also run against the exact base above, with
only the new default-path test copied into an isolated base checkout. The same
`cargo test --locked --test cli quickstart_local` invocation produced:

```text
Base: error: unrecognized subcommand 'quickstart' (exit 2)
Candidate: test quickstart_local_needs_no_server_or_issued_token_and_resumes ... ok
```

The fixture probe distinguishes the CLI change; the real README proof above
separately demonstrates that the actual pinned tools install and verify.
