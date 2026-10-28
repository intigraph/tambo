# Tambo agent primer

## What Tambo is

Tambo installs and proves the Quipu stack: quipu, camayoc, bobbin, yupana,
desire-path and the stack members (seeds, chaski, shuttle), plus a crew runtime.
It writes a reviewable plan, converges the machine to it, and proves each tool
by version read-back and a functional round trip. It never does a tool's job.

## Agent workflow

1. Run `tambo prime` to see which stack tools are installed and their state.
2. Run `tambo doctor` before an install; it changes nothing.
3. Write or review a plan (`tambo plan`, `tambo init --guided`) before `tambo apply`.
4. Prove the result with `tambo verify`; trust its output over an exit code.
5. Use `tambo check-updates` before `tambo update`.

## Core commands

| Command | Use |
| --- | --- |
| `tambo prime` | Print this primer and each installed tool's `prime --brief` |
| `tambo quickstart` | Install the local single-user stack with no interview |
| `tambo doctor` | Report what would stop an install, without changing anything |
| `tambo init --guided` | Run the resumable interview and write a plan |
| `tambo plan --profile <p>` | Write a reviewable plan for a profile |
| `tambo apply` / `tambo verify` | Converge to the plan, then prove each tool works |
| `tambo install` | Apply and verify in one step |
| `tambo check-updates` / `tambo update` | Report and converge drift from reviewed releases |
| `tambo update-self` | Update Tambo from a checksummed release |

Run `tambo help <command>` for the exact syntax of any command.

## MCP tools

None. Tambo registers the stack's MCP servers (`tambo project-settings`) but serves none.

## Next steps

- Book: <https://intigraph.github.io/tambo/>
- The stack prime contract: <https://github.com/intigraph/tambo/blob/main/docs/stack/PRIME-CONTRACT.md>
- Skill: <https://github.com/intigraph/tambo/blob/main/skills/tambo/SKILL.md>
