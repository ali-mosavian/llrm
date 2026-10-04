"""Code size and expected work of every program, two builds side by side.

    python3 tools/sizes.py BASE_BIN_DIR [NEW_BIN_DIR] [-O2|-Os|-Oz|-O3]... [--frames]

Each directory holds llrm-qb, llrm-c and llrm-nib, each one whole compiler (NEW
defaults to target/release). A program is tests/run/qb, bench and examples, plus the
QuickBASIC demos in $QBDEMOS (~/work/qbdemos/orig). Bytes are the OMF object's code segments' (`code_bytes`);
instructions and memory operands are the backend's `cost` estimate per call,
summed (not a timing). With --frames, also the bytes each program reserves below BP
(every `sub sp, N` of its -S listing). Prints the programs that changed and the totals.
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


sys.path.insert(0, str(Path(__file__).resolve().parent / "dosbatch"))

from run_tests import compiler_arguments as settings  # noqa: E402


def programs(bins: Path, demos: bool = True) -> list[tuple[str, list[str]]]:
    qb, cc, nib = (str(bins / name) for name in ("llrm-qb", "llrm-c", "llrm-nib"))
    out = []
    if demos:
        if not DEMOS.is_dir():
            sys.exit(f"{DEMOS} is missing: set QBDEMOS, or the demos cannot be counted")
        out = [(f"demo-{one}", [qb, str(DEMOS / one / "TSC.BAS"), "--dialect", "qb45", "--runtime", "qb45"]) for one in ("qbdemo", "oimad", "deedlines")]
    for pattern, tool in (("tests/run/qb/*.bas", qb), ("bench/*/*.bas", qb), ("bench/parity/*/*.bas", qb), ("bench/*/*.c", cc), ("bench/parity/*/*.c", cc),
                          ("bench/*/*.nib", nib), ("bench/parity/*/*.nib", nib), ("examples/*.nib", nib)):
        out += [(f"{Path(file).parent.name}/{Path(file).name}", [tool, file, *settings(file)]) for file in sorted(glob.glob(str(ROOT / pattern)))]
    return out


def omf_index(body: bytes, at: int) -> tuple[int, int]:
    """An OMF index and where it ends: one byte, or two where the first has its high bit set."""
    if body[at] & 0x80:
        return ((body[at] & 0x7F) << 8) | body[at + 1], at + 2
    return body[at], at + 1


def code_bytes(obj: Path) -> int:
    """Bytes of the object's code segments (a class named ...CODE): its SEGDEF lengths. The file's size counts
    the fixup, symbol and debug records as well, which the linker consumes and the program lacks."""
    data = Path(obj).read_bytes()
    names: list[str] = []
    total = 0
    at = 0
    while at + 3 <= len(data):
        kind, length = data[at], int.from_bytes(data[at + 1 : at + 3], "little")
        body = data[at + 3 : at + 2 + length]
        at += 3 + length
        if kind == 0x96:  # LNAMES
            i = 0
            while i < len(body):
                names.append(body[i + 1 : i + 1 + body[i]].decode("latin-1"))
                i += 1 + body[i]
        elif kind in (0x98, 0x99):  # SEGDEF
            wide = kind == 0x99
            attributes, i = body[0], 1
            if attributes >> 5 == 0:
                i += 3  # frame number and offset
            size = int.from_bytes(body[i : i + (4 if wide else 2)], "little")
            i += 4 if wide else 2
            if attributes & 2:
                size = 1 << 16  # the 'big' bit: a full segment
            _, i = omf_index(body, i)  # the segment's name
            klass, i = omf_index(body, i)
            if names[klass - 1].upper().endswith("CODE"):  # CODE, and the BASIC runtime's BC_CODE
                total += size
    return total


def measure(command: list[str], level: str) -> tuple[int, int, int] | None:
    with tempfile.TemporaryDirectory() as directory:
        obj = f"{directory}/x.obj"
        done = subprocess.run([*command, "--cpu", "486", level, "-o", obj], capture_output=True, text=True, env={**environ, "LLRM_DEBUG": "cost"}, timeout=300)
        if not Path(obj).exists():
            return None
        found = [tuple(map(int, one)) for one in COST.findall(done.stderr)]
        return code_bytes(Path(obj)), sum(i for i, _ in found), sum(m for _, m in found)


SUB_SP = re.compile(r"^\s*sub sp, (\d+)\s*$", re.M)


def frame(command: list[str], level: str) -> int | None:
    """Bytes the program's procedures reserve below BP: every `sub sp, N` of its listing."""
    with tempfile.TemporaryDirectory() as directory:
        listing = f"{directory}/x.s"
        subprocess.run([*command, "--cpu", "486", level, "-S", "-o", listing], capture_output=True, text=True, timeout=300)
        return sum(int(one) for one in SUB_SP.findall(Path(listing).read_text())) if Path(listing).exists() else None


def frames(bins: Path, level: str, demos: bool = True) -> dict:
    with ThreadPoolExecutor(8) as pool:
        listed = programs(bins, demos)
        return dict(zip((name for name, _ in listed), pool.map(lambda one: frame(one[1], level), listed)))


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
                (b0, i0, m0), (b1, i1, m1) = before[name], after[name]
                print(f"{name:34} bytes {b0:>7} -> {b1:>7} ({b1 - b0:+})  ins {i1 - i0:+}  mem {m1 - m0:+}")
        totals = [sum(one[at] for one in (before[n] for n in both)) for at in range(3)], [sum(one[at] for one in (after[n] for n in both)) for at in range(3)]
        print("TOTAL bytes %d -> %d  ins %d -> %d  mem %d -> %d" % tuple(x for pair in zip(*totals) for x in pair))
        if "--frames" in sys.argv:
            before, after = frames(base, level), frames(new, level)
            both = [name for name in before if before[name] is not None and after.get(name) is not None]
            for name in both:
                if before[name] != after[name]:
                    print(f"{name:34} frame {before[name]:>6} -> {after[name]:>6} ({after[name] - before[name]:+})")
            print("TOTAL frame %d -> %d" % (sum(before[n] for n in both), sum(after[n] for n in both)))
        status |= bool(failed)
    sys.exit(status)


if __name__ == "__main__":
    main()
