"""Code size and expected work of every program, two builds side by side.

    python3 tools/sizes.py BASE_BIN_DIR [NEW_BIN_DIR] [-O2|-Os|-Oz|-O3]...

Each directory holds llrm-qb, llrm-c and llrm-nib (NEW defaults to
target/release). A program is tests/suite, bench and examples, plus the
QuickBASIC demos in $QBDEMOS (~/work/qbdemos/orig). Bytes are the OMF object's;
instructions and memory operands are the backend's `cost` estimate per call,
summed (not a timing). Prints the programs that changed and the totals.
"""

import re
import sys
import glob
import tempfile
import subprocess
from os import environ
from pathlib import Path
from concurrent.futures import ThreadPoolExecutor

ROOT = Path(__file__).resolve().parent.parent
DEMOS = Path(environ.get("QBDEMOS", Path.home() / "work/qbdemos/orig"))
COST = re.compile(r"executes (\d+) instructions, (\d+) memory operands")


def programs(bins: Path) -> list[tuple[str, list[str]]]:
    qb, cc, nib = (str(bins / name) for name in ("llrm-qb", "llrm-c", "llrm-nib"))
    out = [(f"demo-{one}", [qb, str(DEMOS / one / "TSC.BAS"), "--dialect", "qb45", "--runtime", "qb45"]) for one in ("qbdemo", "oimad", "deedlines")]
    if not DEMOS.is_dir():
        sys.exit(f"{DEMOS} is missing: set QBDEMOS, or the demos cannot be counted")
    for pattern, tool in (("tests/suite/*.bas", qb), ("bench/*.bas", qb), ("bench/general/*.BAS", qb), ("bench/parity/*.bas", qb), ("bench/c/*.c", cc), ("bench/general/*.c", cc), ("bench/parity/*.c", cc), ("examples/*.nib", nib)):
        out += [(f"{Path(file).parent.name}/{Path(file).name}", [tool, file]) for file in sorted(glob.glob(str(ROOT / pattern)))]
    return out


def measure(command: list[str], level: str) -> tuple[int, int, int] | None:
    with tempfile.TemporaryDirectory() as directory:
        obj = f"{directory}/x.obj"
        done = subprocess.run([*command, "--cpu", "486", level, "-o", obj], capture_output=True, text=True, env={**environ, "LLRM_DEBUG": "cost"}, timeout=300)
        if not Path(obj).exists():
            return None
        found = [tuple(map(int, one)) for one in COST.findall(done.stderr)]
        return Path(obj).stat().st_size, sum(i for i, _ in found), sum(m for _, m in found)


def table(bins: Path, level: str) -> dict:
    with ThreadPoolExecutor(8) as pool:
        listed = programs(bins)
        return dict(zip((name for name, _ in listed), pool.map(lambda one: measure(one[1], level), listed)))


def main() -> None:
    args = [one for one in sys.argv[1:] if not one.startswith("-O")]
    levels = [one for one in sys.argv[1:] if one.startswith("-O")] or ["-O2"]
    base = Path(args[0])
    new = Path(args[1]) if len(args) > 1 else ROOT / "target" / "release"
    status = False
    for level in levels:
        before, after = table(base, level), table(new, level)
        both = [name for name in before if before[name] and after.get(name)]
        failed = [(side, name) for side, built in (("base", before), ("new", after)) for name in built if not built[name]]
        print(f"== {level}: {len(both)} programs")
        for side, name in failed:
            print(f"FAILED to compile on {side}: {name}")
        for name in both:
            if before[name] != after[name]:
                (b0, i0, m0), (b1, i1, m1) = before[name], after[name]
                print(f"{name:34} bytes {b0:>7} -> {b1:>7} ({b1 - b0:+})  ins {i1 - i0:+}  mem {m1 - m0:+}")
        totals = [sum(one[at] for one in (before[n] for n in both)) for at in range(3)], [sum(one[at] for one in (after[n] for n in both)) for at in range(3)]
        print("TOTAL bytes %d -> %d  ins %d -> %d  mem %d -> %d" % tuple(x for pair in zip(*totals) for x in pair))
        status |= bool(failed)
    sys.exit(status)


main()
