#!/usr/bin/env python3
"""Every `owner NAME` in crates, docs and tools whose NAME is not in tools/owners.txt: a row nobody on the bus owns is never removed.

deviate_owners.py [--all]: prints `ID | file:line | owner | exit` of the rows whose owner is not live (--all: the live ones too).
Exit 1 when any is dead. ID is the `DEVIATE X` or `Row X` the line names ("-" when it names none). A word after `owner` that is a worker id (`w-6`) or a hyphenated name
(`funcattrs-port`) must be in owners.txt; plain words (`owner of the pass`) are prose and ignored (the bus names without `llrm-` and `@host`, and short aliases).
"""
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
OWNER = re.compile(r"\bowners?[ :]+(?P<name>[A-Za-z0-9][A-Za-z0-9_-]*)")
ID = re.compile(r"(?:DEVIATE|Row)\s+(?P<id>[A-Za-z0-9][A-Za-z0-9_.-]*)")
EXIT = re.compile(r"exit[ :]+(?P<date>\d{4}-\d\d-\d\d)")
NAME_SHAPED = re.compile(r"^(?:llrm-)?w-?\d+$|^[A-Za-z0-9]+(?:-[A-Za-z0-9]+)+$")  # a worker id (w-6, w12) or a hyphenated session/task name; English words are neither


def live() -> set[str]:
    names = set()
    for line in (ROOT / "tools/owners.txt").read_text().splitlines():
        if line.strip() and not line.startswith("#"):
            names.add(line.split("#")[0].strip())
    return names


def rows():
    hits = subprocess.run(
        ["git", "grep", "-nIE", r"\bowners?[ :]+[A-Za-z0-9]", "--", "crates", "docs", "tools", ":!tools/deviate_owners.py", ":!tools/owners.txt"],
        cwd=ROOT, capture_output=True, text=True,
    ).stdout.splitlines()
    for hit in hits:
        name, number, text = hit.split(":", 2)
        ident, date = ID.search(text), EXIT.search(text)
        for match in OWNER.finditer(text):
            if NAME_SHAPED.match(match["name"]):
                yield ident["id"] if ident else "-", f"{name}:{number}", match["name"], date["date"] if date else "(none)"


def main(argv: list[str]) -> int:
    known, bad = live(), 0
    for ident, where, owner, date in sorted(rows(), key=lambda r: (r[2], r[1])):
        dead = owner not in known
        bad += dead
        if dead or "--all" in argv:
            print(f"{ident} | {where} | {owner} | {date} | {'DEAD' if dead else 'live'}")
    print(f"dead rows: {bad}", file=sys.stderr)
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
