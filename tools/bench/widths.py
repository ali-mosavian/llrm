"""Integer widths of a Nib benchmark against its C twin, from the IR each hands to the backend at m16.

A Nib program that types every int i32 where C's `int` is 16 bits is a different program (jpeg_fixed read 80% i32 integer
operations against C's 20%, 838 bytes larger): `flagged` names the pairs whose Nib i32 share is over C's by more than
THRESHOLD points. Run `widths.py [PROGRAM ...]` for the table; it exits 1 when a pair is flagged.
"""
from __future__ import annotations

import re
import sys
import argparse
import tempfile
import subprocess
from pathlib import Path
from collections import Counter

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import llrmbin  # noqa: E402

THRESHOLD = 25
OPERATIONS = re.compile(r"\s*(?:%\S+ = )?(?:tail )?(?:add|sub|mul|and|or|xor|shl|lshr|ashr|icmp|select|sdiv|srem|udiv|urem)\b[^,]*?\b(i8|i16|i32|i64)\b")
RUNTIME = re.compile(r'@"?(?:std\.|_?llrm|LL\$|N\$)')


def widths(ir: str) -> Counter:
    """The width of each integer operation in the program's own functions (the library's and the runtime's left out)."""
    found: Counter = Counter()
    own = False
    for line in ir.splitlines():
        if line.startswith("define"):
            own = not RUNTIME.search(line.split("(")[0])
        elif line.startswith("}"):
            own = False
        elif own and (match := OPERATIONS.match(line)):
            found[match.group(1)] += 1
    return found


def share(found: Counter) -> float:
    return 100 * found["i32"] / max(1, sum(found.values()))


def flagged(pairs: dict[str, tuple[Counter, Counter]]) -> list[str]:
    """`name: Nib i32 share N% against C's M%` for each pair whose Nib share is over C's by more than THRESHOLD points."""
    return [
        f"{name}: Nib i32 share {share(nib):.0f}% against C's {share(c):.0f}%"
        for name, (nib, c) in sorted(pairs.items())
        if share(nib) - share(c) > THRESHOLD
    ]


def last_stage(directory: Path) -> str:
    return max(directory.glob("*.ll")).read_text()


def pair(nib: Path, c: Path, work: Path, bin_dir: Path) -> tuple[Counter, Counter]:
    """The widths of `nib` and of `c` after the optimiser at -O2, m16."""
    (work / "n").mkdir(parents=True)
    (work / "c").mkdir()
    run = lambda command, **extra: subprocess.run(command, capture_output=True, text=True, check=True, **extra)  # noqa: E731
    run([str(bin_dir / "llrm-nib"), str(nib), "--dump", str(work / "n"), "-O2", "--procedure-segments", "-o", str(work / "n.obj")])
    run([str(bin_dir / "llrm-cc"), *llrmbin.c_args(16), str(c), "-I", str(c.parent), "-c", "-O2", "--dump", str(work / "c"), "-o", str(work / "c.obj")], env={**llrmbin.cc_environment(), "PATH": "/usr/bin:/bin"})
    return widths(last_stage(work / "n" / "mir")), widths(last_stage(work / "c"))


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("programs", nargs="*")
    args = parser.parse_args()
    bench = llrmbin.REPO / "bench"
    names = args.programs or sorted(one.name for one in bench.iterdir() if (one / f"{one.name}.nib").exists() and (one / f"{one.name}.c").exists())
    pairs = {}
    base = llrmbin.target_dir() / "widths"  # a dump is thousands of files: not on tmpfs
    base.mkdir(parents=True, exist_ok=True)
    for name in names:
        with tempfile.TemporaryDirectory(dir=base) as where:
            pairs[name] = pair(bench / name / f"{name}.nib", bench / name / f"{name}.c", Path(where), llrmbin.bin_dir())
        nib, c = pairs[name]
        print(f"{name:13} Nib i16/i32 {nib['i16']:5}/{nib['i32']:5} ({share(nib):3.0f}%)  C {c['i16']:5}/{c['i32']:5} ({share(c):3.0f}%)")
    bad = flagged(pairs)
    print(*bad, sep="\n")
    sys.exit(1 if bad else 0)


if __name__ == "__main__":
    main()
