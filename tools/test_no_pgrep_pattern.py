"""A script that waits for, samples or ends a process by a command-line pattern (`pgrep -f`, `pkill -f`) also matches every session whose prompt or command line names it
(`pkill -f merge-batch` took three sessions down in one day): scripts record a pid and wait on it with `kill -0`."""
from __future__ import annotations

import re
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
# a command, not a word in a comment or a docstring: after a line start, `;`, `&`, `|`, `(` or `$(`, or as the first items of a Python list
PATTERN = re.compile(
    r"(?:^|[;&|(]|\$\()\s*(?:pgrep|pkill)\s+(?:-[a-zA-Z]*f|--full)\b" r"|\[\s*[\"'](?:pgrep|pkill)[\"']\s*,\s*[\"'](?:-[a-zA-Z]*f|--full)[\"']"
)
# tests that name the pattern on purpose, to prove a kill by pattern would have hit a bystander
DEMONSTRATIONS = {"tools/landing/test-stop.sh", "tools/landing/test-check-kills.sh", "tools/landing/check-kills.sh"}


def test_no_script_waits_on_or_kills_by_a_command_line_pattern():
    found = {}
    for path in [*(ROOT / "tools").rglob("*.sh"), *(ROOT / "tools").rglob("*.py"), *ROOT.glob("crates/*/*/*.sh")]:
        name = path.relative_to(ROOT).as_posix()
        if name in DEMONSTRATIONS or name == Path(__file__).relative_to(ROOT).as_posix():
            continue
        lines = [n for n, line in enumerate(path.read_text().splitlines(), 1) if PATTERN.search(line) and not line.lstrip().startswith("#")]
        if lines:
            found[name] = lines
    assert not found, f"a pgrep/pkill by pattern (record a pid and `kill -0` it): {found}"
