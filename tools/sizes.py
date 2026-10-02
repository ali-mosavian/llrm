"""Code size and expected work of every program, two builds side by side.

    python3 tools/sizes.py BASE_BIN_DIR [NEW_BIN_DIR] [-O2|-Os|-Oz|-O3]...

Each directory holds llrm-qb, llrm-c and llrm-nib, each one whole compiler (NEW
defaults to target/release). A program is tests/suite, bench and examples, plus the
QuickBASIC demos in $QBDEMOS (~/work/qbdemos/orig). Bytes are the OMF object's;
instructions and memory operands are the backend's `cost` estimate per call,
summed (not a timing); frame is the bytes the procedures reserve below BP (every
`sub sp, N` of the listing). $QCPORT and $QCPORT_INC (tools/qcport-compile.sh's) add
QCport's modules. Prints the programs that changed and the totals.
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


def programs(bins: Path, demos: bool = True) -> list[tuple[str, list[str]]]:
    qb, cc, nib = (str(bins / name) for name in ("llrm-qb", "llrm-c", "llrm-nib"))
    out = []
    if demos:
        if not DEMOS.is_dir():
            sys.exit(f"{DEMOS} is missing: set QBDEMOS, or the demos cannot be counted")
        out = [(f"demo-{one}", [qb, str(DEMOS / one / "TSC.BAS"), "--dialect", "qb45", "--runtime", "qb45"]) for one in ("qbdemo", "oimad", "deedlines")]
    for pattern, tool in (("tests/suite/*.bas", qb), ("bench/*.bas", qb), ("bench/general/*.BAS", qb), ("bench/parity/*.bas", qb), ("bench/c/*.c", cc), ("bench/general/*.c", cc), ("bench/parity/*.c", cc), ("examples/*.nib", nib)):
        out += [(f"{Path(file).parent.name}/{Path(file).name}", [tool, file]) for file in sorted(glob.glob(str(ROOT / pattern)))]
    out += qcport(cc)
    return out


def qcport(cc: str) -> list[tuple[str, list[str]]]:
    """QCport's modules, when $QCPORT names its src/ and $QCPORT_INC the Borland headers it builds with."""
    source, headers = environ.get("QCPORT"), environ.get("QCPORT_INC")
    if not source or not headers:
        return []
    include = [flag for one in ("host", "render", "model", "game", "sound", "ui", "qgl") for flag in ("-I", f"{source}/{one}")] + ["-I", headers]
    modules = sorted(glob.glob(f"{source}/host/*.c") + glob.glob(f"{source}/render/*.c") + glob.glob(f"{source}/model/*.c") + glob.glob(f"{source}/game/*.c") + glob.glob(f"{source}/sound/*.c") + glob.glob(f"{source}/ui/*.c"))
    return [(f"qcport/{Path(one).name}", [cc, one, *include]) for one in modules]


def measure(command: list[str], level: str) -> tuple[int, int, int, int] | None:
    """Object bytes, expected instructions and memory operands, and the bytes the procedures
    reserve below BP (every `sub sp, N` of the listing the compile dumps beside its stages)."""
    with tempfile.TemporaryDirectory() as directory:
        obj, stages = f"{directory}/x.obj", f"{directory}/stages"
        done = subprocess.run([*command, "--cpu", "486", level, "-o", obj], capture_output=True, text=True, env={**environ, "LLRM_DEBUG": "cost", "LLRM_MIR_STAGES": stages}, timeout=1800)
        if not Path(obj).exists():
            return None
        found = [tuple(map(int, one)) for one in COST.findall(done.stderr)]
        listing = Path(stages) / "listing.asm"
        frame = sum(int(one) for one in SUB_SP.findall(listing.read_text())) if listing.exists() else 0
        return Path(obj).stat().st_size, sum(i for i, _ in found), sum(m for _, m in found), frame


SUB_SP = re.compile(r"^\s*sub sp, (\d+)\s*$", re.M)


def table(bins: Path, level: str, demos: bool = True) -> dict:
    with ThreadPoolExecutor(8) as pool:
        listed = programs(bins, demos)
        return dict(zip((name for name, _ in listed), pool.map(lambda one: measure(one[1], level), listed)))


def main() -> None:
    args = [one for one in sys.argv[1:] if not one.startswith("-")]
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
                (b0, i0, m0, f0), (b1, i1, m1, f1) = before[name], after[name]
                print(f"{name:34} bytes {b0:>7} -> {b1:>7} ({b1 - b0:+})  ins {i1 - i0:+}  mem {m1 - m0:+}  frame {f1 - f0:+}")
        totals = [sum(one[at] for one in (before[n] for n in both)) for at in range(4)], [sum(one[at] for one in (after[n] for n in both)) for at in range(4)]
        print("TOTAL bytes %d -> %d  ins %d -> %d  mem %d -> %d  frame %d -> %d" % tuple(x for pair in zip(*totals) for x in pair))
        status |= bool(failed)
    sys.exit(status)


if __name__ == "__main__":
    main()
