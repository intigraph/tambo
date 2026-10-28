#!/usr/bin/env python3
"""Install, remove and verify camayoc's memory-file redirect hook (aegis-gshu.1).

camayoc ships the hook (`cy hook memory-redirect`, see camayoc
docs/installation-reference.md). This step wires it into a harness, and only
when asked: nothing in `tambo install` enables it.

    memory-redirect-hook.py install   --harness claude|codex|both [--mode copy|block] [--person IRI]
    memory-redirect-hook.py uninstall --harness claude|codex|both
    memory-redirect-hook.py status    --harness claude|codex|both
    memory-redirect-hook.py verify    --harness claude|codex|both [--live]

Claude Code: a `PreToolUse` group on `Write|Edit|MultiEdit` in
`~/.claude/settings.json` (or `--claude-settings`). Codex: a `PreToolUse`
group on `apply_patch` (Codex's native file edits) appended to
`$CODEX_HOME/config.toml` (or `--codex-config`) between marker comments, so
the operator's comments and key order survive and uninstall removes exactly
that block. Codex runs a hook only after it has been trusted; see `status`.

Install is idempotent: it replaces its own entry, keeps every other hook, and
copies the file to `<file>.before-memory-redirect` before its first change.
Changing the mode is another `install --mode ...`.

`verify` reads the entry back, then runs the configured command on a synthetic
memory-file write inside a scratch directory, with `CAMAYOC_REDIRECT_VERIFY`
set, so the hook maps and prechecks the file but never contacts a store.
block mode must refuse with a `cy` call; copy mode must return at once and its
background worker must log the probe. `--live` (Claude only) also runs one
`claude -p` session in a scratch HOME whose only settings are this hook in
block mode (needs CLAUDE_CODE_OAUTH_TOKEN or ANTHROPIC_API_KEY, since a scratch
HOME has no login), and asserts that Claude itself invoked the hook, that the
file was not written and that nothing landed in the real ~/.claude.

Install needs a person to record rulings under: --person, $CAMAYOC_PERSON,
or `person` under [memory] in ~/.config/camayoc/config.toml (--no-person to
install without one).
"""
from __future__ import annotations

import argparse
import json
import os
import re
import shlex
import shutil
import socket
import subprocess
import sys
import tempfile
import time
import uuid
from pathlib import Path

try:
    import tomllib
except ModuleNotFoundError:  # Python < 3.11: Codex support needs tomllib to read back
    tomllib = None

HOOK_SUBCOMMAND = "hook memory-redirect"
CLAUDE_MATCHER = "Write|Edit|MultiEdit"
CODEX_MATCHER = "apply_patch"
MODES = ("copy", "block")
BEGIN = "# >>> tambo: camayoc memory-redirect hook (aegis-gshu.1) >>>"
END = "# <<< tambo: camayoc memory-redirect hook <<<"
BACKUP_SUFFIX = ".before-memory-redirect"
TIMEOUT_S = 10
INSTALL_HINT = "install camayoc first: uv tool install git+https://github.com/intigraph/camayoc"


class Refusal(RuntimeError):
    pass


# ---- the command ----------------------------------------------------------------------

def hook_command(cy: str, mode: str, person: str | None) -> str:
    """The hook's shell command.

    `cy` is looked up at run time: the install-time path first (as found on
    PATH, never symlink-resolved, so a Homebrew or uv upgrade that moves the
    real file keeps working), then `cy` on the hook's PATH. No cy at all exits
    0, so the agent is never broken; otherwise exec, so block's exit 2 stands.
    """
    if mode not in MODES:
        raise Refusal(f"mode must be one of {', '.join(MODES)}")
    if person is not None and not re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9._:/#~-]{0,199}", person):
        raise Refusal("--person must be an entity IRI or local name")
    q = shlex.quote(cy)
    tail = f" --person {shlex.quote(person)}" if person else ""
    return (f'c={q}; [ -x "$c" ] || c=$(command -v cy) || exit 0; '
            f'exec "$c" {HOOK_SUBCOMMAND} --mode {mode}{tail}')


def is_ours(command: str) -> bool:
    return HOOK_SUBCOMMAND in (command or "")


def mode_of(command: str) -> str | None:
    m = re.search(r"--mode (\w+)", command or "")
    return m.group(1) if m else None


def camayoc_config() -> Path:
    base = os.environ.get("XDG_CONFIG_HOME") or str(Path.home() / ".config")
    return Path(base) / "camayoc/config.toml"


def resolve_person(explicit: str | None, allow_none: bool) -> str | None:
    """--person, else $CAMAYOC_PERSON, else `person` under [memory] in ~/.config/camayoc/config.toml.

    Without one the worker refuses every directive, decision and preference,
    so install refuses unless --no-person says that is intended.
    """
    if explicit:
        return explicit
    if os.environ.get("CAMAYOC_PERSON"):
        return os.environ["CAMAYOC_PERSON"]
    cfg = camayoc_config()
    if cfg.exists() and tomllib is not None:
        try:
            person = (tomllib.loads(cfg.read_text(encoding="utf-8")).get("memory") or {}).get("person")
        except tomllib.TOMLDecodeError:
            raise Refusal(f"{cfg} does not parse as TOML; nothing was changed") from None
        if person:
            return str(person)
    if allow_none:
        return None
    raise Refusal("no person to record rulings and preferences under: pass --person <IRI>, set CAMAYOC_PERSON, "
                  f"or put person = \"<IRI>\" under [memory] in {cfg}. Without one every directive, decision "
                  "and preference is refused; pass --no-person to install anyway")


def resolve_cy(explicit: str | None) -> str:
    cy = explicit or shutil.which("cy")
    if not cy:
        raise Refusal("cy is not on PATH; " + INSTALL_HINT)
    path = Path(os.path.abspath(os.path.expanduser(cy)))  # keep the shim/symlink, never its target
    if not (path.is_file() and os.access(path, os.X_OK)):
        raise Refusal(f"{cy} is not an executable file; " + INSTALL_HINT)
    try:
        out = subprocess.run([str(path), "hook", "memory-redirect", "--help"], capture_output=True, text=True,
                             timeout=20)
    except (OSError, subprocess.TimeoutExpired) as exc:
        raise Refusal(f"{cy} did not answer: {type(exc).__name__}") from None
    if out.returncode != 0 or "--mode" not in out.stdout:
        raise Refusal(f"{cy} has no `hook memory-redirect`; upgrade camayoc ({INSTALL_HINT})")
    return str(path)


# ---- files --------------------------------------------------------------------------------

def default_claude_settings() -> Path:
    return Path.home() / ".claude/settings.json"


def default_codex_config() -> Path:
    return Path(os.environ.get("CODEX_HOME") or Path.home() / ".codex") / "config.toml"


def _backup(path: Path) -> None:
    backup = path.with_name(path.name + BACKUP_SUFFIX)
    if path.exists() and not backup.exists():
        shutil.copy2(path, backup)


def _write_atomic(path: Path, text: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    fd, tmp = tempfile.mkstemp(prefix=f".{path.name}.", dir=path.parent)
    with os.fdopen(fd, "w", encoding="utf-8") as fh:
        fh.write(text)
    if path.exists():
        os.chmod(tmp, path.stat().st_mode & 0o777)
    os.replace(tmp, path)


# ---- Claude settings.json ------------------------------------------------------------------

def _indent_of(path: Path):
    """The file's own indent (spaces or a tab), default 2, so a rewrite is a minimal diff."""
    try:
        for line in path.read_text(encoding="utf-8").splitlines()[1:]:
            lead = line[: len(line) - len(line.lstrip())]
            if lead:
                return "\t" if lead.startswith("\t") else len(lead)
    except OSError:
        pass
    return 2


def _claude_dump(path: Path, data: dict) -> str:
    return json.dumps(data, indent=_indent_of(path), ensure_ascii=False) + "\n"


def _claude_load(path: Path) -> dict:
    if not path.exists():
        return {}
    try:
        data = json.loads(path.read_text(encoding="utf-8") or "{}")
    except json.JSONDecodeError as exc:
        raise Refusal(f"{path} is not valid JSON (line {exc.lineno}); fix it first, nothing was changed") from None
    if not isinstance(data, dict):
        raise Refusal(f"{path} is not a settings object; nothing was changed")
    return data


def _strip_ours(groups: list) -> list:
    out = []
    for g in groups:
        if not isinstance(g, dict):
            out.append(g)
            continue
        hooks = [h for h in g.get("hooks", []) if not (isinstance(h, dict) and is_ours(h.get("command", "")))]
        if hooks:
            out.append({**g, "hooks": hooks})
        elif not g.get("hooks"):
            out.append(g)
    return out


def claude_entries(path: Path) -> list[dict]:
    data = _claude_load(path)
    found = []
    for g in (data.get("hooks") or {}).get("PreToolUse") or []:
        for h in (g.get("hooks") or []) if isinstance(g, dict) else []:
            if isinstance(h, dict) and is_ours(h.get("command", "")):
                found.append({"matcher": g.get("matcher"), "command": h.get("command")})
    return found


def claude_install(path: Path, command: str) -> str:
    data = _claude_load(path)
    hooks = data.setdefault("hooks", {})
    if not isinstance(hooks, dict):
        raise Refusal(f"{path}: `hooks` is not an object; nothing was changed")
    groups = _strip_ours(hooks.get("PreToolUse") or [])
    groups.append({"matcher": CLAUDE_MATCHER, "hooks": [{"type": "command", "command": command,
                                                          "timeout": TIMEOUT_S}]})
    hooks["PreToolUse"] = groups
    _backup(path)
    _write_atomic(path, _claude_dump(path, data))
    return f"claude: PreToolUse {CLAUDE_MATCHER} -> {command} ({path})"


def claude_uninstall(path: Path) -> str:
    if not claude_entries(path):
        return f"claude: no memory-redirect hook in {path}"
    data = _claude_load(path)
    groups = _strip_ours(data["hooks"].get("PreToolUse") or [])
    if groups:
        data["hooks"]["PreToolUse"] = groups
    else:
        del data["hooks"]["PreToolUse"]
        if not data["hooks"]:
            del data["hooks"]
    _backup(path)
    _write_atomic(path, _claude_dump(path, data))
    return f"claude: removed the memory-redirect hook from {path}"


# ---- Codex config.toml ---------------------------------------------------------------------

def _toml_str(s: str) -> str:
    return json.dumps(s)  # a JSON string is a valid TOML basic string for this character set


def _codex_block(command: str) -> str:
    return (f"{BEGIN}\n[[hooks.PreToolUse]]\nmatcher = {_toml_str(CODEX_MATCHER)}\n\n"
            f"[[hooks.PreToolUse.hooks]]\ntype = \"command\"\ncommand = {_toml_str(command)}\n"
            f"timeout = {TIMEOUT_S}\n{END}\n")


def _codex_without_block(text: str) -> str:
    return re.sub(rf"\n?{re.escape(BEGIN)}\n.*?{re.escape(END)}\n?", "\n", text, flags=re.S).rstrip("\n") + "\n" \
        if BEGIN in text else text


def _codex_parse(text: str, path: Path) -> dict:
    if tomllib is None:
        raise Refusal("Codex support needs Python 3.11+ (tomllib) to read config.toml back")
    try:
        return tomllib.loads(text)
    except tomllib.TOMLDecodeError as exc:
        raise Refusal(f"{path} would not parse as TOML ({exc}); nothing was changed") from None


def codex_entries(path: Path) -> list[dict]:
    if not path.exists():
        return []
    data = _codex_parse(path.read_text(encoding="utf-8"), path)
    found = []
    for g in ((data.get("hooks") or {}).get("PreToolUse") or []):
        for h in g.get("hooks") or []:
            if is_ours(h.get("command", "")):
                found.append({"matcher": g.get("matcher"), "command": h.get("command")})
    return found


def codex_install(path: Path, command: str) -> str:
    text = path.read_text(encoding="utf-8") if path.exists() else ""
    current = _codex_parse(text, path)
    pre = (current.get("hooks") or {}).get("PreToolUse")
    if pre is not None and not isinstance(pre, list):
        raise Refusal(f"{path}: hooks.PreToolUse is not an array of tables; add the hook by hand")
    base = _codex_without_block(text)
    new = (base.rstrip("\n") + "\n\n" if base.strip() else "") + _codex_block(command)
    parsed = _codex_parse(new, path)  # refuse to write a file codex would reject
    ours = [h for g in parsed["hooks"]["PreToolUse"] for h in g.get("hooks", []) if is_ours(h.get("command", ""))]
    if len(ours) != 1:
        raise Refusal(f"{path}: found {len(ours)} memory-redirect hooks after the edit; nothing was changed")
    _backup(path)
    _write_atomic(path, new)
    return (f"codex: PreToolUse {CODEX_MATCHER} -> {command} ({path}); Codex runs it only once trusted "
            "(approve it at the next launch, or launch with --dangerously-bypass-hook-trust)")


def codex_uninstall(path: Path) -> str:
    if not path.exists() or BEGIN not in path.read_text(encoding="utf-8"):
        return f"codex: no memory-redirect hook block in {path}"
    new = _codex_without_block(path.read_text(encoding="utf-8"))
    _codex_parse(new, path)
    _backup(path)
    _write_atomic(path, new)
    return f"codex: removed the memory-redirect hook block from {path}"


# ---- verify -------------------------------------------------------------------------------------

def _closed_port() -> int:
    s = socket.socket()
    s.bind(("127.0.0.1", 0))
    port = s.getsockname()[1]
    s.close()
    return port


PROBE = """---
name: tambo-verify
description: Directive - verify probe written by tambo; never recorded
metadata:
  type: feedback
  camayoc:
    entity: directive
    rule: Verify the memory-redirect hook fires.
    issuedBy: tambo-verify
    appliesTo: crew
    topic: tambo-verify
---
Probe body.
"""


def _probe_payload(harness: str, path: Path) -> dict:
    if harness == "codex":
        patch = "*** Begin Patch\n*** Add File: " + str(path) + "\n" + "".join(
            "+" + line + "\n" for line in PROBE.splitlines()) + "*** End Patch"
        return {"hook_event_name": "PreToolUse", "tool_name": "apply_patch", "tool_input": {"command": patch},
                "cwd": str(path.parent)}
    return {"hook_event_name": "PreToolUse", "tool_name": "Write",
            "tool_input": {"file_path": str(path), "content": PROBE}, "cwd": str(path.parent)}


def _read_log(state: Path) -> list[dict]:
    log = state / "camayoc/memory-redirect.jsonl"
    if not log.exists():
        return []
    rows = []
    for line in log.read_text(encoding="utf-8").splitlines():
        try:
            rows.append(json.loads(line))
        except json.JSONDecodeError:
            continue
    return rows


def fire(harness: str, command: str, *, wait_s: float = 20.0) -> list[str]:
    """Run the configured command on a synthetic memory write; raise Refusal unless it behaves per its mode."""
    mode = mode_of(command)
    nonce = "tambo-verify-" + uuid.uuid4().hex[:12]
    with tempfile.TemporaryDirectory(prefix="tambo-memredirect-") as tmp:
        root = Path(tmp)
        target = root / "home/.claude/projects/tambo-verify/memory/tambo-verify.md"
        target.parent.mkdir(parents=True)
        state = root / "state"
        env = {k: v for k, v in os.environ.items() if k != "CAMAYOC_MEMORY_REDIRECT"}
        env.pop("CLAUDE_CONFIG_DIR", None)
        env.update({"HOME": str(root / "home"), "XDG_STATE_HOME": str(state), "CAMAYOC_REDIRECT_VERIFY": nonce,
                    "QUIPU_SERVER": f"http://127.0.0.1:{_closed_port()}"})
        start = time.monotonic()
        try:
            proc = subprocess.run(["sh", "-c", command], input=json.dumps(_probe_payload(harness, target)),
                                  capture_output=True, text=True, env=env, cwd=str(root), timeout=TIMEOUT_S + 5)
        except subprocess.TimeoutExpired:
            raise Refusal(f"{harness}: the hook did not return within {TIMEOUT_S + 5}s") from None
        took = time.monotonic() - start
        fired = [r for r in _read_log(state) if r.get("event") == "fired" and r.get("verify") == nonce]
        if not fired:
            raise Refusal(f"{harness}: the hook ran (exit {proc.returncode}) but logged no `fired` event; "
                          f"is cy on PATH and new enough? stderr: {proc.stderr.strip()[:200]!r}")
        if mode == "block":
            if proc.returncode != 2 or "cy record" not in proc.stderr:
                raise Refusal(f"{harness}: block mode must exit 2 naming a `cy record` call; got exit "
                              f"{proc.returncode}")
            return [f"{harness}: fired in block mode; the write was refused with: "
                    + proc.stderr.strip().splitlines()[1].strip()[:160]]
        if proc.returncode != 0 or proc.stdout.strip() or proc.stderr.strip():
            raise Refusal(f"{harness}: copy mode must exit 0 silently; got exit {proc.returncode}")
        deadline = time.monotonic() + wait_s
        while time.monotonic() < deadline:
            done = [r for r in _read_log(state) if r.get("verify") == nonce and r.get("event") != "fired"]
            if done:
                r = done[-1]
                if r.get("event") != "verified":
                    raise Refusal(f"{harness}: the background worker logged {r.get('event')}: {r}")
                return [f"{harness}: fired in copy mode, returned in {took:.2f}s; the background worker mapped the "
                        f"probe to a {r.get('entity')} (precheck exit {r.get('code')}, store never contacted)"]
            time.sleep(0.2)
        raise Refusal(f"{harness}: copy mode returned but its background worker logged nothing in {wait_s:.0f}s")


LIVE_CREDENTIALS = ("CLAUDE_CODE_OAUTH_TOKEN", "ANTHROPIC_API_KEY")


def _real_claude_projects() -> set[str]:
    root = Path.home() / ".claude/projects"
    try:
        return {p.name for p in root.iterdir()}
    except OSError:
        return set()


def live_claude(command: str, claude: str = "claude", timeout: int = 240) -> list[str]:
    """One real Claude Code session in a scratch HOME whose only settings are this hook, forced to block.

    HOME and CLAUDE_CONFIG_DIR point at a scratch directory, so no user
    settings, hooks, plugins or transcripts are read or written: nothing lands
    in the real ~/.claude. A scratch HOME has no login (the keychain entry is
    tied to the real config dir), so the session needs CLAUDE_CODE_OAUTH_TOKEN
    (from `claude setup-token`) or ANTHROPIC_API_KEY in the environment.
    """
    exe = shutil.which(claude)
    if not exe:
        raise Refusal("--live needs the claude CLI on PATH")
    creds = {k: os.environ[k] for k in LIVE_CREDENTIALS if os.environ.get(k)}
    if not creds:
        raise Refusal("--live runs Claude Code in a scratch HOME, which has no login: set CLAUDE_CODE_OAUTH_TOKEN "
                      "(claude setup-token) or ANTHROPIC_API_KEY in this shell")
    nonce = "tambo-live-" + uuid.uuid4().hex[:12]
    before = _real_claude_projects()
    with tempfile.TemporaryDirectory(prefix="tambo-memredirect-live-") as tmp:
        root = Path(tmp)
        home = root / "home"
        work = root / "work"
        work.mkdir(parents=True)
        target = home / ".claude/projects/tambo-live/memory/tambo-live.md"
        target.parent.mkdir(parents=True)
        settings = root / "settings.json"
        settings.write_text(json.dumps({"hooks": {"PreToolUse": [{"matcher": CLAUDE_MATCHER, "hooks": [
            {"type": "command", "command": command, "timeout": TIMEOUT_S}]}]}}))
        state = root / "state"
        env = {"PATH": os.environ.get("PATH", "/usr/bin:/bin"), "HOME": str(home),
               "CLAUDE_CONFIG_DIR": str(home / ".claude"), "XDG_STATE_HOME": str(state),
               "XDG_CONFIG_HOME": str(home / ".config"), "TMPDIR": str(root),
               "CAMAYOC_REDIRECT_VERIFY": nonce, "CAMAYOC_MEMORY_REDIRECT": "block",
               "QUIPU_SERVER": f"http://127.0.0.1:{_closed_port()}", **creds}
        prompt = (f"Use the Write tool exactly once to create the file {target} with the content "
                  f"'---\\nname: tambo-live\\n---\\nprobe\\n'. If the write is refused, stop and reply DONE.")
        try:
            proc = subprocess.run([exe, "-p", prompt, "--settings", str(settings), "--setting-sources", "",
                                   "--allowedTools", "Write", "--max-turns", "3"], capture_output=True, text=True,
                                  env=env, cwd=str(work), timeout=timeout)
        except subprocess.TimeoutExpired:
            raise Refusal(f"claude --live: the session did not finish in {timeout}s") from None
        if "not logged in" in (proc.stdout + proc.stderr).lower():
            raise Refusal("claude --live: the scratch session is not logged in; check the credential in "
                          + "/".join(creds))
        fired = [r for r in _read_log(state) if r.get("event") == "fired" and r.get("verify") == nonce]
        if not fired:
            raise Refusal("claude --live: Claude Code did not invoke the hook (no `fired` event for this session)")
        if target.exists():
            raise Refusal("claude --live: the hook fired but the memory file was still written")
    leaked = {p for p in _real_claude_projects() - before if "tambo-memredirect-live" in p}
    if leaked:
        raise Refusal(f"claude --live: the session left {sorted(leaked)} in the real ~/.claude/projects")
    return [f"claude --live: Claude Code (scratch HOME, only this hook) invoked it {len(fired)}x; the memory file "
            "was not written and nothing was left in the real ~/.claude"]


# ---- commands --------------------------------------------------------------------------------------

def _harnesses(value: str) -> list[str]:
    return ["claude", "codex"] if value == "both" else [value]


def run(argv: list[str]) -> int:
    ap = argparse.ArgumentParser(prog="memory-redirect-hook.py", description=__doc__.split("\n\n")[0])
    ap.add_argument("action", choices=("install", "uninstall", "status", "verify"))
    ap.add_argument("--harness", choices=("claude", "codex", "both"), required=True)
    ap.add_argument("--mode", choices=MODES, default="copy", help="install only (default copy)")
    ap.add_argument("--person", help="install only: who issues the rulings in memory files (entity IRI)")
    ap.add_argument("--no-person", action="store_true", help="install only: install without a person (most "
                    "records will be refused)")
    ap.add_argument("--cy", help="install only: path to cy (default: cy on PATH)")
    ap.add_argument("--claude-settings", type=Path, default=None)
    ap.add_argument("--codex-config", type=Path, default=None)
    ap.add_argument("--live", action="store_true", help="verify only: also prove it through a real claude -p run")
    args = ap.parse_args(argv)
    paths = {"claude": args.claude_settings or default_claude_settings(),
             "codex": args.codex_config or default_codex_config()}
    entries = {"claude": claude_entries, "codex": codex_entries}
    failed = False
    try:
        if args.action == "install":
            person = resolve_person(args.person, args.no_person)
            command = hook_command(resolve_cy(args.cy), args.mode, person)
            if person is None:
                print("warning: installed without --person; directives, decisions and preferences will be refused")
            for h in _harnesses(args.harness):
                print((claude_install if h == "claude" else codex_install)(paths[h], command))
            return 0
        if args.action == "uninstall":
            for h in _harnesses(args.harness):
                print((claude_uninstall if h == "claude" else codex_uninstall)(paths[h]))
            return 0
        for h in _harnesses(args.harness):
            found = entries[h](paths[h])
            if args.action == "status":
                if not found:
                    print(f"{h}: off (no memory-redirect hook in {paths[h]})")
                for e in found:
                    print(f"{h}: {mode_of(e['command'])} on {e['matcher']} ({paths[h]})")
                if os.environ.get("CAMAYOC_MEMORY_REDIRECT"):
                    print(f"note: CAMAYOC_MEMORY_REDIRECT={os.environ['CAMAYOC_MEMORY_REDIRECT']} in this shell "
                          "overrides the configured mode wherever the harness inherits it")
                continue
            try:
                want = CLAUDE_MATCHER if h == "claude" else CODEX_MATCHER
                if len(found) != 1:
                    raise Refusal(f"{h}: expected exactly one memory-redirect hook in {paths[h]}, found {len(found)}")
                if found[0]["matcher"] != want:
                    raise Refusal(f"{h}: the hook's matcher is {found[0]['matcher']!r}, expected {want!r}")
                print(f"OK {h}: configured, {mode_of(found[0]['command'])} mode on {want} ({paths[h]})")
                for line in fire(h, found[0]["command"]):
                    print("OK " + line)
                if h == "codex":
                    print("NOTE codex: delivery by the codex binary itself is not exercised here; Codex must trust "
                          "the hook and support native apply_patch PreToolUse events")
                if args.live and h == "claude":
                    for line in live_claude(found[0]["command"]):
                        print("OK " + line)
            except Refusal as exc:
                failed = True
                print(f"FAIL {exc}")
        return 1 if failed else 0
    except Refusal as exc:
        print(f"refused: {exc}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    sys.exit(run(sys.argv[1:]))
