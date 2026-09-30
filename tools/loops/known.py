"""
The quality ratchet: known.toml lists every (case, language, configuration,
check) that falls short today, each with its issue. A run fails for a
shortfall not listed and for a listed one that no longer falls short, within
the cases, languages and configurations it ran. A pass PR may shrink the
list, never grow it.
"""

from __future__ import annotations

import tomllib
from pathlib import Path
from dataclasses import dataclass

PATH = Path(__file__).resolve().parent / "known.toml"
HEADER = """\
# What falls short today, per check (tools/loops/run.py). Remove an entry when
# it no longer falls short; never add one without its issue.
"""


@dataclass
class Ratchet:
    new: set
    fixed: set
    kept_issues: dict


def load() -> tuple[set, dict]:
    if not PATH.exists():
        return set(), {}
    data = tomllib.loads(PATH.read_text())
    entries, issues = set(), {}
    for one in data.get("short", []):
        for config in one["configs"]:
            key = (one["case"], one["lang"], config, one["check"])
            entries.add(key)
            issues[(one["case"], one["lang"], one["check"])] = one.get("issue", "")
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


def write(short: set, issues: dict, default: str = "#98") -> None:
    """known.toml as `short` stands, entries outside it kept."""
    kept = PATH.read_text().split("[[short]]")[0].replace(HEADER, "").strip() if PATH.exists() else ""
    grouped: dict[tuple, set] = {}
    for case, lang, config, check in short:
        grouped.setdefault((case, lang, check), set()).add(config)
    lines = [HEADER, kept, ""] if kept else [HEADER]
    # the dot example first: issue #98's own evidence
    for (case, lang, check), configs in sorted(grouped.items(), key=lambda kv: (kv[0][0] != "dot", kv[0])):
        lines += ["[[short]]", f'case = "{case}"', f'lang = "{lang}"', f'check = "{check}"',
                  "configs = [" + ", ".join(f'"{c}"' for c in sorted(configs)) + "]",
                  f'issue = "{issues.get((case, lang, check), default)}"', ""]
    PATH.write_text("\n".join(lines))
