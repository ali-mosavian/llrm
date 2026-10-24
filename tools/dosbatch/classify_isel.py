#!/usr/bin/env python3
"""Sorts the FAIL lines of a `LLRM_ISEL=dag run_tests.py` log into causes, each failing program in exactly one row.

    classify_isel.py RUN.log        a markdown table: cause, owner, count, programs

The rows sum to the FAIL count, or the tool refuses: a failure no rule names is an unclassified remainder (exit 1).
"""

from __future__ import annotations

import re
import sys
from collections import defaultdict
from pathlib import Path

FAIL = re.compile(r"^FAIL  (?P<program>[^\s:]+)(?P<flags>(?:\s*\[[^\]]*\])*): (?P<message>.*)$", re.S)

# (pattern on the message, cause (with groups), owner), first match wins
RULES: list[tuple[re.Pattern[str], str, str]] = [
    (re.compile(r"--isel=dag: CannotSelect\(\"(\w+)\"\)"), "unselected node {0}", "sdag-c"),
    (re.compile(r"--isel=dag: Emit\((?:Unsupported|Unmapped)\(\"(.*?)\"\)\)"), "emit: {0}", "sdag-d"),
    (re.compile(r"--isel=dag: Legalize\((.*)\)"), "legalize: {0}", "sdag-b"),
    (re.compile(r"--isel=dag: (library call whose symbol and convention .*)"), "builder: {0}", "sdag-a"),
    (re.compile(r"--isel=dag: (a switch lowering|register arguments or a switch lowering)"), "harness: {0}", "sdag-d"),
    (re.compile(r"--isel=dag: value \d+ is read and not defined on every path \((\w+\(\"?\w*\"?\)|\w+)\)"), "emit: a value read before it is defined on every path ({0})", "sdag-d"),
    (re.compile(r"--isel=dag: (.+)"), "builder refusal: {0}", "sdag-a"),
    # a panic of the compiler: the file names the owner (see PANIC_OWNERS)
    (re.compile(r"panicked at crates/\S*?/(selectiondag/[\w/]+\.rs):\d+:\d+:\s*(.*)", re.S), "panic in {0}: {1}", "by file"),
    (re.compile(r"Semantics\(op=<Operation\.CALL.*indirect=True\)"), "lir: a direct call is emitted as an indirect call of an immediate", "sdag-d"),
    (re.compile(r"isel: \S+ names \S+ at width \d+, which is not the width that register is"), "lir verify: a register named at the wrong width", "sdag-d"),
    (re.compile(r"isel: value#\d+ is read but never defined"), "lir verify: a value read and never defined", "sdag-d"),
    (re.compile(r"isel: value#\d+ is defined \d+ times"), "lir verify: a value defined more than once", "sdag-d"),
    (re.compile(r"Semantics\(op=<Operation\.ADDRESS"), "lir: an address form the verifier rejects", "sdag-d"),
    (re.compile(r"floatalloc: \S+ reads \w+ before every path writes it"), "floatalloc: a register read before any path writes it", "sdag-d"),
    (re.compile(r"the greedy allocator could not place every range"), "allocator: could not place every range", "regalloc"),
    (re.compile(r"jwlink: Error! E2028: (\S+) is an undefined reference"), "link: {0} undefined", "sdag-b"),
    (re.compile(r"^line \d+: want '.*', got '.*'$"), "wrong output", "triage"),
    (re.compile(r"^(limit|timeout|stopped|over the output cap)\b"), "hang or stopped", "triage"),
]


# the owner of a selectiondag source file, for a panic in it
PANIC_OWNERS = {"builder": "sdag-a", "dag.rs": "sdag-a", "emit": "sdag-d", "legalize": "sdag-b", "x86_lowering": "sdag-b", "dagisel": "sdag-c"}


def owner_of(file: str) -> str:
    return next((owner for key, owner in PANIC_OWNERS.items() if key in file), "triage")


def normalized(text: str) -> str:
    """The cause with its numbers and function names folded, so a row is a kind, not an instance."""
    return re.sub(r"-?\d+", "N", re.sub(r"@[\w$@.?]+", "@F", text))


def blocks(log: str) -> list[str]:
    """Each FAIL line with the lines that continue its message (an error spans several)."""
    out: list[str] = []
    for line in log.splitlines():
        if line.startswith("FAIL"):
            out.append(line)
        elif out and not re.match(r"^\d+ programs:|^isel proof|^stamp|^done ", line):
            out[-1] += "\n" + line
    return out


def classify(log: str) -> tuple[dict[tuple[str, str], list[str]], list[str]]:
    """The programs by (cause, owner), and the FAIL lines no rule names."""
    rows: dict[tuple[str, str], list[str]] = defaultdict(list)
    unclassified = []
    for line in blocks(log):
        found = FAIL.match(line)
        if not found:
            unclassified.append(line)
            continue
        for pattern, cause, owner in RULES:
            hit = pattern.search(found["message"])
            if hit:
                who = owner_of(hit.group(1)) if owner == "by file" else owner
                rows[(normalized(cause.format(*(g.strip().splitlines()[0] for g in hit.groups()))), who)].append(found["program"] + found["flags"])
                break
        else:
            unclassified.append(line)
    return rows, unclassified


def table(log: str) -> str:
    rows, unclassified = classify(log)
    if unclassified:
        raise SystemExit("unclassified failures (add a rule):\n" + "\n".join(unclassified))
    failed = len(blocks(log))
    total = sum(len(programs) for programs in rows.values())
    assert total == failed, (total, failed)
    out = [f"| cause | owner | n | programs |", "|---|---|---|---|"]
    for (cause, owner), programs in sorted(rows.items(), key=lambda one: (-len(one[1]), one[0])):
        out.append(f"| {cause} | {owner} | {len(programs)} | {', '.join(programs)} |")
    out.append(f"| total | | {total} | |")
    return "\n".join(out)


if __name__ == "__main__":
    print(table(Path(sys.argv[1]).read_text()))
