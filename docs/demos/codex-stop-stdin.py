"""Replay the embedded base/head hook transport against one private fixture."""

import errno
import json
from pathlib import Path
import shlex
import shutil
import sqlite3
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[2]
BASE = "e076a8309a07b29d317429df3a91083adb1e98fe"
before = json.loads(subprocess.check_output(
    ["git", "show", f"{BASE}:hook-bundles/desire-path.json"], cwd=ROOT
))
after = json.loads((ROOT / "hook-bundles/desire-path.json").read_text())
dp = shutil.which("dp")
assert dp, "dp >= 0.10.0 must be installed"
print("Installed consumer:", subprocess.check_output([dp, "version"], text=True).strip())
with tempfile.TemporaryDirectory() as directory:
    db = Path(directory) / "fixture.db"
    event = {"hook_event_name": "Stop", "session_id": "ox0-demo-fixture",
             "turn_id": "fixture-turn", "cwd": directory,
             "last_assistant_message": "x" * 262144}
    payload = json.dumps(event)
    print("Same synthetic Stop payload: 262144 message bytes")
    print("BEFORE: execute base bundle's notify argv with appended payload")
    try:
        subprocess.run(before["codex_notify"] + [payload], check=True,
                       capture_output=True)
    except OSError as error:
        assert error.errno == errno.E2BIG, error
        print("Observed E2BIG before the consumer could start; no record")
    else:
        raise AssertionError("expected Linux per-argument limit")
    assert not db.exists()
    hooks = [h for h in after["hooks"] if h["event"] == "Stop"
             and h["harnesses"] == ["codex"]]
    assert len(hooks) == 1 and "codex_notify" not in after
    command = shlex.split(hooks[0]["command"])
    assert command == ["dp", "ingest", "--source", "codex"]
    # The global --db option isolates storage; the bundle command is unchanged.
    command = [dp, "--db", str(db)] + command[1:]
    print("AFTER: execute head bundle's Stop command with payload on stdin")
    result = subprocess.run(command, input=payload, text=True, capture_output=True)
    assert result.returncode == 0, result.stderr
    with sqlite3.connect(db) as connection:
        rows = connection.execute(
            "select source, tool_name, instance_id from invocations"
        ).fetchall()
        assert rows == [("codex", "agent_turn", "ox0-demo-fixture")], rows
        print("Exit 0; exactly one codex/agent_turn record with fixture session")
        event["hook_event_name"] = "PreToolUse"
        rejected = subprocess.run(command, input=json.dumps(event), text=True,
                                  capture_output=True)
        assert rejected.returncode != 0
        assert connection.execute("select count(*) from invocations").fetchone()[0] == 1
        print("Negative control: non-Stop event refused; record count remains one")
print("PASS: private transport demonstration; no live hook reconfiguration")
