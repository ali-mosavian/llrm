"""A script that waits for, samples or ends a process by a command-line pattern also matches every session whose prompt or command line names it (a kill by pattern took
three sessions down in one day): scripts record a pid and wait on it. (The words of the commands are built from parts below: tools/landing/check-kills.sh, which
tests/no_kill_by_pattern.rs runs over every tracked tool, flags a line that spells them out.)"""
from __future__ import annotations

import re
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
FIND, END = "pg" + "rep", "pk" + "ill"
FLAG = r"(?:-[a-zA-Z]*f|--full)"
# a command, not a word in a comment or a docstring: after a line start, a separator or `$(`, or as the first items of a Python list
PATTERN = re.compile(rf"(?:^|[;&|(]|\$\()\s*(?:{FIND}|{END})\s+{FLAG}\b" rf"|\[\s*[\"'](?:{FIND}|{END})[\"']\s*,\s*[\"']{FLAG}[\"']")
# tests that name the pattern on purpose, to prove a pattern would have hit a bystander
DEMONSTRATIONS = {"tools/landing/test-stop.sh", "tools/landing/test-check-kills.sh", "tools/landing/check-kills.sh"}


def test_no_script_waits_on_or_ends_a_process_by_a_command_line_pattern():
    found = {}
    for path in [*(ROOT / "tools").rglob("*.sh"), *(ROOT / "tools").rglob("*.py"), *ROOT.glob("crates/*/*/*.sh")]:
        name = path.relative_to(ROOT).as_posix()
        if name in DEMONSTRATIONS or name == Path(__file__).relative_to(ROOT).as_posix():
            continue
        lines = [n for n, line in enumerate(path.read_text().splitlines(), 1) if PATTERN.search(line) and not line.lstrip().startswith("#")]
        if lines:
            found[name] = lines
    assert not found, f"a process found or ended by pattern (record a pid and check it with kill -0): {found}"
