"""
The quality ratchet: shortfalls.txt lists every (case, language, configuration,
check) that falls short today, each with its issue, one line per (case,
language, check), written only by `run.py --write-known`. known.toml holds the
hand-written patterns of known bugs. A run fails for a
shortfall not listed and for a listed one that no longer falls short, within
the cases, languages and configurations it ran. A pass PR may shrink the
list, never grow it.
"""

from __future__ import annotations

import tomllib
from pathlib import Path
from dataclasses import dataclass

PATH = Path(__file__).resolve().parent / "known.toml"
SHORTFALLS = Path(__file__).resolve().parent / "shortfalls.txt"
HEADER = """\
# What falls short today: case language check issue configuration...
# Written by tools/loops/run.py --write-known; never add a line without its issue.
"""


@dataclass
class Ratchet:
    new: set
    fixed: set
    kept_issues: dict


def load() -> tuple[set, dict]:
    if not SHORTFALLS.exists():
        return set(), {}
    entries, issues = set(), {}
    for line in SHORTFALLS.read_text().splitlines():
        if not line.strip() or line.startswith("#"):
            continue
        case, lang, check, issue, *configs = line.split()
        entries |= {(case, lang, config, check) for config in configs}
        issues[(case, lang, check)] = issue
    return entries, issues


def bugs() -> list[tuple[str, str]]:
    """(a regular expression a wrong or unbuilt line matches, its issue):
    known llrm bugs, named in the report so a new failure stands out. They
    still fail the run."""
    if not PATH.exists():
        return []
    return [(one["match"], one["issue"]) for one in tomllib.loads(PATH.read_text()).get("bug", [])]


def compare(short: set, judged: set) -> Ratchet:
    """Against known.toml, within the checks this run evaluated."""
    entries, issues = load()
    scope = entries & judged
    return Ratchet(short - scope, scope - short, issues)


def write(short: set, issues: dict, judged: set | None = None, default: str = "#98") -> None:
    """shortfalls.txt as `short` stands; entries for checks this run did not
    evaluate (`judged`) are kept."""
    if judged is not None:
        short = (load()[0] - judged) | short
    grouped: dict[tuple, set] = {}
    for case, lang, config, check in short:
        grouped.setdefault((case, lang, check), set()).add(config)
    lines = [HEADER.rstrip("\n")]
    # the dot example first: issue #98's own evidence
    for (case, lang, check), configs in sorted(grouped.items(), key=lambda kv: (kv[0][0] != "dot", kv[0])):
        lines.append(" ".join([case, lang, check, issues.get((case, lang, check), default), *sorted(configs)]))
    SHORTFALLS.write_text("\n".join(lines) + "\n")
