#!/usr/bin/env python3
"""Print the actual generated plan's review boundary, without applying it."""
import tomllib
from pathlib import Path
p = tomllib.loads(Path("caboodle-plan.toml").read_text())
print("Profile:", p["profile"])
print("Would install:", ", ".join(p["tools"]))
print("Purpose:", p["intent"]["intended_use"])
print("Question:", p["intent"]["anticipated_questions"][0]["question"])
print("Review the full caboodle-plan.toml before applying.")
