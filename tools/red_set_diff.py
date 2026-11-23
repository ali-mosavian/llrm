#!/usr/bin/env python3
"""Red names of one `cargo test --no-fail-fast` log against a tip log.

usage: red_set_diff.py TIP.log HEAD.log

Exit 0: no red name only in HEAD. 1: names only in HEAD (printed). 2: the
instrument is broken, a test could not find a build product ("No such file"
under a target dir), so the sets say nothing about the code.
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

RED = re.compile(r"^test (\S+) \.\.\. FAILED$", re.M)
MISSING = re.compile(r"^.*(?:target[^\s:]*|/release|/debug)/[^\s:]*: No such file or directory.*$", re.M)


def red(text: str) -> set[str]:
    return set(RED.findall(text))


def missing_products(text: str) -> list[str]:
    return sorted(set(MISSING.findall(text)))


def main(argv: list[str]) -> int:
    tip, head = (Path(a).read_text(errors="replace") for a in argv[1:3])
    for name, text in (("tip", tip), ("head", head)):
        lines = missing_products(text)
        if lines:
            print(f"instrument defect: {name} log has {len(lines)} missing build product(s), e.g. {lines[0].strip()}")
            return 2
    only = sorted(red(head) - red(tip))
    for name in only:
        print(name)
    return 1 if only else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
