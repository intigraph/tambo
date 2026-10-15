# Using it with agents

Caboodle is a CLI that an agent can run with the
[Tambo skill](https://github.com/scbrown/tambo/blob/main/skills/tambo/SKILL.md).
It does not expose its own MCP server. MCP (Model Context Protocol) lets an
agent call servers supplied by the installed tools.

## Standalone agent

After installing the retrieval profile, register Bobbin with Claude Code. Run it
from the repository to index, and register by **absolute path**: MCP clients launch
servers without your shell's `PATH`, so a bare `bobbin` fails with `ENOENT`:

```bash
claude mcp add bobbin -- "$(command -v bobbin)" serve "$PWD"
```

For code-intel or everything, also register Yupana:

```bash
claude mcp add yupana -- "$(command -v yupana)" serve
claude mcp list
```

Index a repository before searching it: run `bobbin init` and `bobbin index`
in that repository. Other MCP clients use the same executable and arguments
in their client-specific server configuration.

## Managed crew

For a configured Shantytown rig, `caboodle project-settings` delegates
registration to Shantytown and its Quipu tooling manifest. It does not launch
or restart agents. See [registering MCP servers](installing.md#registering-mcp-servers)
for prerequisites, selection flags, remote proxies and the policy-only mode.

## Agent identity and write access

Agents that write to a shared Quipu store sign each write with their own key.
No shared bearer is sent. The operator who runs the store decides which keys may
write, and can withdraw that permission for one agent without touching any
other.

### One identity per agent

Run each agent as its own operating-system user. At minimum, give each agent its
own `HOME` and its own `~/.config`. Never share one login between agents. This
buys you three things:

- **Attribution.** Every write records the agent that signed it, so the history
  says who did what.
- **Revocation.** You can revoke one agent's key and leave every other agent
  working.
- **Blast radius.** A leaked key exposes one agent's write permission, and only
  until you revoke it.

Never copy a key file between hosts or between agents. A second machine, or a
second agent on the same machine, gets its own key.

### 1. Create the key on the agent's machine

As the agent's user, run `sd key init`. The introducer is the person or lead who
will register the key on the Quipu host. It is never the agent itself.

```bash
sd key init --introducer <operator> --session <agent>-<host> --agent <agent>
```

`sd key init` writes one owner-only (mode `0600`) private key file under
`~/.config/seeds/keys/` and never overwrites an existing key. It prints:

- the **public key** and its `key_id`;
- the lines to add to `~/.config/seeds/config.toml`;
- the one-time `quipu attest register` command for your operator.

Only the public key leaves the machine. `sd key show` prints the same details
again later.

Add the printed lines to the agent's user config, together with the server and
project the agent works in:

```toml
[quipu]
url = "https://quipu.example.org"
signing_key_file = "/home/<agent>/.config/seeds/keys/<agent>-<host>.key"
signing_session = "<agent>-<host>"
signing_introducer = "<operator>"

[project]
graph = "https://example.org/project/<name>"
prefix = "<prefix>"
```

Until the key is registered, a write is refused with a message that names the
fix:

```text
signed write refused (unbound). The server has no binding for this key and
session: run `sd key show` and have your introducer run the `quipu attest
register` line it prints.
```

### 2. Register the key on the Quipu host

The operator runs the printed command once, as the user that owns the store,
and adds `--db` with the store path:

```bash
quipu attest register --agent <agent> --session <agent>-<host> \
  --public-key <hex> --introducer <operator> \
  --issued-at <epoch> --expires-at <epoch> --allow-write \
  --db /path/to/quipu.db
quipu attest list --db /path/to/quipu.db
```

`attest list` shows one row per binding, with its `key_id`, introducer,
`allow_write`, expiry and `revoked` state. The running server picks up a new
binding without a restart.

- **Share-only vs write.** Without `--allow-write`, a binding only lets the
  store trust shares that this key signed. With it, the key may also sign writes
  to `/knot`, `/update` and `/episode`.
- **Expiry.** Writes stop at `--expires-at`. `sd key init` proposes about one
  year; choose what your policy allows.
- **CLI version.** Use the `quipu` CLI whose version matches the served server
  (compare `quipu --version` with the server's `/version`). Server packages
  that ship only `quipu-server` do not include `attest`.

### 3. Verify

As the agent, make one write, then read it back as someone else:

```bash
sd create "agent write check"           # as the agent: prints the new id
sd show <id> --json                     # as the operator: created_by is the agent
```

The server log records the accepted write with `auth_outcome`
`authenticated_attested`.

### 4. Lifecycle

| Action | Command (on the Quipu host, with `--db`) | Agent sees |
|---|---|---|
| Withdraw write, keep share trust | `quipu attest deny-write <session>` | `signed write refused (scope)` |
| Restore write | `quipu attest allow-write <session>` | writes succeed again |
| Revoke the key | `quipu attest revoke <session>` | `signed write refused (revoked)` |
| Rotate | the agent runs `sd key init` with a **new** session; register it, then revoke the old session | writes succeed with the new key |

Revocation is immediate and permanent for that session. To replace a key, mint a
new one under a new session name: `sd key init` refuses to overwrite an
existing key file.

### Pitfalls

- **`quipu-server` has no `attest` subcommand.** It ignores the word and starts
  a server on its default store. Use the `quipu` CLI on the store host.
- **Do not register your own key.** The introducer registers it, so another
  person or lead vouches for each agent.
- **A fallback credential can hide a missing key.** If an old shared-token file,
  or a symlink to one, is still present on the agent's machine, older clients may
  keep authenticating through it. They then never report the missing or rejected
  identity. Remove such fallbacks once every client uses its own key, and confirm
  with `caboodle doctor`, which names the credential source in use.

### Worked example: enrolling an agent on a second host

This is a real enrollment, with names and paths replaced. An operator gave the
agent on a second host write access to a shared store:

1. **On the agent's host**, as the agent's own user: `sd key init --introducer
   <lead> --session <store>-<host>-<agent> --agent <host>-<agent>`. The agent
   sent its lead the printed public key and register line, and nothing else.
2. **On the Quipu host**, the operator checked the served version with
   `curl -s https://quipu.example.org/version`. They then downloaded the `quipu`
   CLI of exactly that version from the published GitHub release, and checked
   its archive against the release's own `.sha256` file. Releases publish one
   checksum file per archive. They unpacked it into a private temporary
   directory, not onto `PATH`, and touched no service.
3. As the user that owns the store, the operator ran `quipu attest list --db
   <store>`, which showed 4 existing rows. Then they ran the register line once,
   with `--allow-write` and `--db <store>`. A fresh `attest list` showed 5 rows:
   the new binding with `allow_write=true revoked=false`, and the 4 earlier
   rows unchanged. They deleted the temporary directory.
4. The agent wrote one comment. From a third machine, the lead read it back with
   the agent as its author.
5. Rollback, if needed: `quipu attest revoke <session> --db <store>`, run with a
   matching verified CLI.

`--allow-write` grants write permission to the store as a whole. Per-item or
comment-only limits are not part of the binding. Do not assume them.
