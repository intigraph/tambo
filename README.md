<p align="center"><img src="assets/header.svg" width="100%" alt="Tambo toolkit"/></p>

# Tambo

Install the Quipu stack from a plan you can review, then prove the selected tools
work. Give an agent durable memory and code context, without assembling every tool
by hand. Tambo is a CLI and [agent skill](skills/tambo/SKILL.md).
The compatibility command `caboodle` remains available.

## Install

```bash
curl -fsSL https://raw.githubusercontent.com/intigraph/tambo/main/install.sh | sh
```

Downloads the latest release, verifies its SHA-256 and version, and installs
`tambo` and `caboodle` in `${CARGO_HOME:-$HOME/.cargo}/bin`. Requires `curl`,
`tar`, Python 3 and `sha256sum` or `shasum`. Add that directory to `PATH`.
Published archives support Linux x86_64 and macOS arm64/x86_64; **Linux arm64
has no published archive**. Source builds, pinned versions, removal and the
Homebrew alternative are in the [installation reference](https://intigraph.github.io/tambo/installation-reference.html).

**Debian and Ubuntu (amd64):** install from the signed apt repository, which
republishes each checksum-verified release. The package puts `tambo` and the
compatibility `caboodle` command in `/usr/bin`. Check the signing key's
fingerprint before trusting it; the commands stop if it does not match:

```bash
curl -fsSL https://scbrown.github.io/apt/key.gpg -o scbrown-stack.gpg \
  && gpg --show-keys --with-colons scbrown-stack.gpg \
  | grep -qx 'fpr:::::::::AB5625C5612F643BF76464DF0DC99924994EC959:' \
  && sudo install -D -m 0644 scbrown-stack.gpg /etc/apt/keyrings/scbrown-stack.gpg \
  && echo "deb [signed-by=/etc/apt/keyrings/scbrown-stack.gpg] https://scbrown.github.io/apt stable main" \
     | sudo tee /etc/apt/sources.list.d/scbrown-stack.list \
  && sudo apt update && sudo apt install tambo
```

`apt upgrade` then picks up new releases. unattended-upgrades only installs
from origins it allows, so to have it upgrade tambo too, add
`"origin=scbrown,codename=stable";` to `Unattended-Upgrade::Origins-Pattern`.
A host with the older `caboodle 0.3.4` package moves over with
`sudo apt install tambo` (or `apt full-upgrade`): `caboodle 0.3.4-1` is a
transitional package that depends on `tambo`.
Channels, pinning and rollback:
[scbrown/apt](https://github.com/scbrown/apt#channels-and-pinning).

## First success: local, single-user

Run in an empty directory you intend to keep:

```bash
tambo quickstart
```

This saves a reviewed retrieval plan, installs Quipu, Camayoc and Bobbin, and
checks each tool. Success is exit 0 and `quipu: verified`, `camayoc: verified`,
`bobbin: verified`. Keep `caboodle-plan.toml` and `.caboodle/` to resume.
Recheck the same install:

```bash
tambo verify
```

### Advanced: shared services and agents

For issued credentials, remote stores and registering Bobbin with your agent,
use the [agent and installation reference](https://intigraph.github.io/tambo/installation-reference.html).
The default quickstart is local and needs no account or remote server.

From the code repository you want searchable, initialize and index it before
registering the server by absolute path:

```bash
bobbin init
bobbin index
claude mcp add bobbin -- "$(command -v bobbin)" serve "$PWD"
```

## Where to go next

- [Choose a profile](https://intigraph.github.io/tambo/profiles.html) or use `tambo init --guided`.
- [Camayoc](https://github.com/scbrown/camayoc): vocabulary, shapes and governed agent memory.
- [Getting started](https://intigraph.github.io/tambo/getting-started.html) and [command reference](https://intigraph.github.io/tambo/reference.html).
- **Load this repository's graph:** repository-pendant publication is pending.
  When a release provides the archive, checksum and provenance, follow Quipu's
  [verified import workflow](https://intigraph.github.io/quipu/sharing/index.html).
  Import stages knowledge; it does not promote it into ROOT automatically.

## The stack

<!-- stack-table:start -->

The Quipu stack is built around a governed, bitemporal knowledge store: shared
facts, decisions and work history your agents can query. Quipu can check writes
against your rules, records where each fact came from, and answers what was true
when.

| tool | what it gives your agents |
|---|---|
| [tambo](https://github.com/intigraph/tambo) | 🧰 Agent Platform Toolkit - installs and configures the quipu stack |
| [quipu](https://github.com/scbrown/quipu) | stores shared facts with provenance and history; can check writes against your rules |
| [camayoc](https://github.com/scbrown/camayoc) | prepares knowledge and vocabulary for Quipu, with checks before it is admitted |
| [quechua](https://github.com/scbrown/quechua) | shared vocabulary and shapes for interoperable knowledge |
| [chaski](https://github.com/scbrown/chaski) | reacts to graph changes and delivers governed notifications |
| [bobbin](https://github.com/scbrown/bobbin) | indexes your codebase and injects context from that index and Quipu knowledge |
| [yupana](https://github.com/scbrown/yupana) | shows code impact and can read policies from Quipu to govern agent actions |
| [desire-path](https://github.com/scbrown/desire-path) | turns agent mistakes into feature requests; works on its own without Quipu |
| [seeds](https://github.com/scbrown/seeds) | a work item tracker backed by the Quipu knowledge base |
| [shantytown](https://github.com/scbrown/shantytown) | runs crews of AI agents on work items, triggered by hand, on a schedule or by events; can share work and memory through Quipu |
| [rikuy](https://github.com/scbrown/rikuy) | live brief: status, graphs, watches and signed decisions over Quipu |
| [shuttle](https://github.com/scbrown/shuttle) | runs signed agent workflows and keeps their records in Quipu |

<!-- stack-table:end -->

## Contributing

Run `just check`, `just book` and `just docs-check`.

[Apache-2.0 license](LICENSE).
