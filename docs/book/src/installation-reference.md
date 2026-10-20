# Installation and agent reference

For Debian/Ubuntu packages, use the [signed apt repository guide](https://github.com/scbrown/apt#channels-and-pinning), including its fingerprint check, allowed update origins and rollback. The archive installer below is independent of apt.


### Checksummed archives

Checksummed releases: Linux x86_64, macOS arm64 and macOS x86_64.
The installer verifies SHA256 before placing the binary in `~/.cargo/bin`.

```bash
curl -fsSL https://raw.githubusercontent.com/scbrown/caboodle/main/scripts/install.sh | bash
```

To pin a release, pass the version to the receiving shell:

```bash
curl -fsSL https://raw.githubusercontent.com/scbrown/caboodle/main/scripts/install.sh | CABOODLE_VERSION=vX.Y.Z bash
```

Add the install directory to this shell's PATH:

```bash
export PATH="${CARGO_HOME:-$HOME/.cargo}/bin:$PATH"
```

The `export` lasts for this shell only. To keep it, add the same line once to
your shell's startup file: `~/.bashrc` for bash, `~/.zshrc` for zsh (the macOS
default). For bash:

```bash
echo 'export PATH="${CARGO_HOME:-$HOME/.cargo}/bin:$PATH"' >> ~/.bashrc
```

Apps started outside a terminal, such as the Claude desktop app or an IDE, may
not read that file, which is why the agent step below registers servers by
absolute path.

Or build the latest release from source with Rust (`cargo install --git
https://github.com/scbrown/caboodle --tag vX.Y.Z --locked`). Either way,
`caboodle --version` prints `caboodle X.Y.Z` for the release you installed.

### macOS with Homebrew

```bash
brew install scbrown/tap/caboodle
caboodle --version
```

Homebrew owns the installed command; Caboodle still composes and verifies the
stack. Update it with `brew upgrade scbrown/tap/caboodle`. To hold the installed
version, use `brew pin caboodle`; resume updates with `brew unpin caboodle`.
The [tap channel guide](https://github.com/scbrown/homebrew-tap#caboodle-preview-channel)
explains the opt-in nightly channel and switching back to stable.

## First success: local, single-user

After installing Tambo, run this in an empty directory you will keep:

```bash
tambo quickstart
```

Tambo saves a retrieval plan, installs Quipu, Camayoc and Bobbin, and verifies
all three. There are no interview questions, issued tokens, accounts or running
server prerequisites. Your knowledge store is local at `.caboodle/quipu.db`.
Keep `caboodle-plan.toml` and `.caboodle/` to resume or update the same install.
An existing plan is reused and validated; quickstart never replaces it.

**Success check:** exit 0 and `quipu: verified`, `camayoc: verified`, and
`bobbin: verified`. Each tool passes an isolated functional round trip, including
a negative control and a read after writing. Recheck with one command:

```bash
tambo verify
```

Quipu's verification uses a temporary embedded database. Camayoc's check starts
and stops its own temporary loopback server; you do not supply or manage one.
This proves the installed tools, not a connection to a remote knowledge service.
Optional Shantytown hook messages do not require a crew installation.

### Advanced: connect to a shared, authenticated Quipu server

Remote mode is opt-in: set `QUIPU_SERVER` to the server you intend to use, or
create a plan with `--quipu-mcp-url`. Obtain an administrator-issued token and
provision it before verification:

```bash
tambo provision-quipu-token --from /secure/issued-token --server https://your-quipu-server.example
export QUIPU_SERVER=https://your-quipu-server.example
tambo verify
```

Remote verification requires the canonical credential with `0400/0700`
permissions, proves that unauthenticated access is refused and the selected
credential is accepted, and refuses token rotation. Missing or rejected
credentials stop verification. Never put a token in the plan or a server URL.
Unset `QUIPU_SERVER` to use the local default; an explicit `quipu_mcp` plan
remains remote until you remove that configuration deliberately.

To customize the tools and intended-use contract instead, use
`tambo init --guided` or `tambo plan --profile retrieval --intent caboodle-intent.toml`,
review the resulting plan, then run `tambo install`. See the
[walkthrough](https://scbrown.github.io/caboodle/getting-started.html).

## On your own code

| I want to… | Command |
|---|---|
| Choose tools and describe my own use | `caboodle init --guided` |
| Find install blockers before changing anything | `caboodle doctor` |
| Install and prove the reviewed selection | `caboodle install` |
| Recheck the selected tools | `caboodle verify` |
| Compare against this build's reviewed versions | `caboodle check-updates` |

[Full command and configuration reference](https://scbrown.github.io/caboodle/reference.html).

## Wire it into your agent

Tambo is a CLI, with an [agent skill](https://github.com/scbrown/tambo/blob/main/skills/tambo/SKILL.md), not an MCP
server. Bobbin, which the `retrieval` profile installed above, is one. It
searches one repository: the code you want your agent to work on, such as your
own project's git checkout. Do not use `~/caboodle`, which holds only the plan.
Index that repository, then register bobbin from inside it:

```bash
cd ~/path/to/your/repo
bobbin init
bobbin index
claude mcp add bobbin -- "$(command -v bobbin)" serve "$PWD"
```

`$(command -v bobbin)` and `$PWD` are expanded once, now, so Claude Code
starts the server by absolute path and serves this repository whatever its own
`PATH` or working directory. Registering a bare `bobbin` fails with `ENOENT` when
Claude Code was not started from a shell that has `~/.cargo/bin` on `PATH`.

`claude mcp add` uses the default `local` scope: the server is registered for
this project only, so start Claude Code in this repository to see it (`claude
mcp list` there shows `bobbin: ... ✓ Connected`). Repeat the four lines in each
repository you want searchable. `bobbin init` creates `.bobbin/`
in the repository and, if the repository has a `.gitignore`, appends `.bobbin/`
to it; review that change before you commit. Without `bobbin init`, `bobbin
serve` exits with `Bobbin not initialized` and Claude Code reports the server as
failed to connect.

MCP (Model Context Protocol) lets your agent call the installed server.
[Agent setup](https://scbrown.github.io/caboodle/agents.html) covers indexing, Yupana, other clients
and managed crew registration through `caboodle project-settings`.

## Before you start

| Platform | Caboodle | Quipu | Yupana |
|---|---|---|---|
| Linux x86_64 | Release | Release | Release |
| macOS arm64 / x86_64 | Release | Release | Release |
| Linux arm64 | Build with Rust | Release | Build with Rust |

Have `curl`, `tar`, `git`, `bash`, `python3` and `sha256sum` on PATH
(`brew install coreutils` supplies `sha256sum` on macOS).
The `everything` profile also needs Go. No account or remote server is needed.
[Installation details](https://scbrown.github.io/caboodle/installing.html) cover source builds,
installed files and removal; `caboodle doctor` names missing prerequisites.

## What's next

- [Read the book](https://scbrown.github.io/caboodle/introduction.html)
- [Find a topic in the docs map](https://scbrown.github.io/caboodle/docs-map.html)
- [Choose your profile](https://scbrown.github.io/caboodle/profiles.html)

