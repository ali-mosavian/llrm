"""The IR's `target-cpu` attributes must equal the CPU the llc is given.

llc honours a function's `target-cpu` over `-mcpu`: a `clang -m32` module says pentium4 (cmov, P4 tuning), so `llc -mcpu=i486` next to
ours at i486 compares two targets. 2026-10-27: every m32 pin diff did, and six hand-offs went out on it.

    irguard.py CPU FILE.ll...        exit 1 and a message on a mismatch
"""
from __future__ import annotations

import re
import sys


def check(text: str, cpu: str, name: str = "<ir>") -> None:
    found = set(re.findall(r'"target-cpu"="([^"]*)"', text))
    bad = found - {cpu}
    if bad:
        raise SystemExit(f"irguard: {name}: target-cpu {sorted(bad)} but the llc is run with {cpu}")


if __name__ == "__main__":
    for path in sys.argv[2:]:
        with open(path) as handle:
            check(handle.read(), sys.argv[1], path)
