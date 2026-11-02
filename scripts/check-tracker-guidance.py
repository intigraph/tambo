#!/usr/bin/env python3
"""Fail when user-facing docs teach beads (br/bd) for work tracking.

Work items are tracked with Seeds (`sd`). beads is a legacy tracker: Tambo can
import its JSONL exports, and an agent's `bd` habit is aliased to `sd`. Those
facts may be documented, but new guidance must not tell anyone to track work
with `br` or `bd`.

Two kinds of line are refused in docs/book/src, README.md and skills/**/*.md:
  1. a br/bd command form such as `br ready` or `bd create`, inline or in a
     code block;
  2. `br` or `bd` named as a tool on the same line as tracking language
     (track, tracker, issue, work item).
A line that must mention the legacy tracker carries the marker
`<!-- legacy-tracker -->` (hidden in the rendered book), so every exception
is visible in review.

# arming: ci  run by the Documentation workflow on every push and pull request.
Usage: check-tracker-guidance.py [ROOT]   exit 0 clean, 1 findings, 2 unreadable
"""
import re
import sys
from pathlib import Path

MARKER = "<!-- legacy-tracker -->"
VERBS = (
    "ready|show|create|update|close|list|comments?|search|dep|label|init|sync|q|"
    "prime|reopen|defer|blocked|stats|count|graph|history|delete|doctor"
)
COMMAND = re.compile(rf"(?<![\w./-])(?:br|bd)\s+(?:{VERBS})\b")
TOOL = re.compile(r"`(?:br|bd)`")
TRACKING = re.compile(r"\b(?:track(?:s|ed|ing|er)?|issues?|work items?)\b", re.IGNORECASE)


def targets(root: Path):
    book = root / "docs/book/src"
    yield from sorted(book.rglob("*.md"))
    if (root / "README.md").is_file():
        yield root / "README.md"
    yield from sorted((root / "skills").rglob("*.md"))


def findings(text: str):
    for number, line in enumerate(text.splitlines(), 1):
        if MARKER in line:
            continue
        if COMMAND.search(line):
            yield number, "br/bd command", line.strip()
        elif TOOL.search(line) and TRACKING.search(line):
            yield number, "br/bd presented as the tracker", line.strip()


def main(argv):
    root = Path(argv[1] if len(argv) > 1 else Path(__file__).resolve().parents[1])
    paths = list(targets(root))
    if not any(p.is_relative_to(root / "docs/book/src") for p in paths):
        print(f"UNKNOWN: no book pages under {root}/docs/book/src", file=sys.stderr)
        return 2
    bad = 0
    for path in paths:
        try:
            text = path.read_text(encoding="utf-8")
        except (OSError, UnicodeDecodeError) as error:
            print(f"UNKNOWN: cannot read {path}: {error}", file=sys.stderr)
            return 2
        for number, kind, line in findings(text):
            bad += 1
            print(f"{path.relative_to(root)}:{number}: {kind}: {line}")
    if bad:
        print(f"{bad} line(s) teach br/bd for work tracking; use sd (Seeds), "
              f"or mark a deliberate legacy mention with {MARKER}", file=sys.stderr)
        return 1
    print(f"tracker guidance clean: {len(paths)} file(s) checked")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
