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


def missing_dso_local(text: str) -> list[str]:
    """Functions and globals a PIC target would reach through the GOT/PLT: no `dso_local`, not `internal`/`private`. The pin's llc is
    PIC on clang IR, ours is not, so a same-IR byte compare at x86-64 reads the GOT/PLT loads as ours being smaller (2026-11-05:
    jpeg_fixed x86_64 read -276 B, really +78 B). Intrinsics are skipped."""
    missing = []
    for line in text.splitlines():
        if line.startswith(("define ", "declare ")):
            head = line.partition("(")[0]
            words, name = head.split(), re.findall(r"@([^\s(]+)", head)
        elif line.startswith("@") and "=" in line:
            left, _, right = line.partition("=")
            words = [word for word in right.split() if word not in ("global", "constant")][:6]
            name = [left.strip()[1:].strip('"')]
        else:
            continue
        if name and not name[-1].startswith("llvm.") and not {"dso_local", "internal", "private"} & set(words):
            missing.append(name[-1])
    return missing


def check_dso_local(text: str, name: str = "<ir>") -> None:
    missing = missing_dso_local(text)
    if missing:
        raise SystemExit(f"irguard: {name}: {len(missing)} symbols without dso_local (first: {missing[0]}); the pin's llc would be PIC and ours not")


SHIFT = re.compile(r"\b((?:shl|lshr|ashr)(?: (?:nuw|nsw|exact))* i(\d+) [^,\n]+, (\d+))\b")


def undefined_shifts(text: str) -> list[str]:
    """Shifts by a constant of at least the type's width: UB, which the pin folds to poison and ours does not (x_fir at m16)."""
    return [found.group(1) for found in SHIFT.finditer(text) if int(found.group(3)) >= int(found.group(2))]


if __name__ == "__main__":
    for path in sys.argv[2:]:
        with open(path) as handle:
            check(handle.read(), sys.argv[1], path)
