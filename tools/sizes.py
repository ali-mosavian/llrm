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
from dataclasses import dataclass
from os import environ
from pathlib import Path
from concurrent.futures import ThreadPoolExecutor

ROOT = Path(__file__).resolve().parent.parent
DEMOS = Path(environ.get("QBDEMOS", Path.home() / "work/qbdemos/orig"))
COST = re.compile(r"executes (\d+) instructions, (\d+) memory operands")


sys.path.insert(0, str(Path(__file__).resolve().parent / "dosbatch"))

from run_tests import compiler_arguments as settings, header  # noqa: E402


def dialect(basic: bool, source: str) -> list[str]:
    """BASIC without a `dialect:` header is built as QuickBASIC 4.5, as tests/run and bench build it; llrm-qb's own default is VBDOS."""
    return ["--dialect", "qb45", "--runtime", "qb45"] if basic and "dialect" not in header(Path(source)) else []


def programs(bins: Path, demos: bool = True) -> list[tuple[str, list[str]]]:
    qb, cc, nib = (str(bins / name) for name in ("llrm-qb", "llrm-c", "llrm-nib"))
    out = []
    if demos:
        if not DEMOS.is_dir():
            sys.exit(f"{DEMOS} is missing: set QBDEMOS, or the demos cannot be counted")
        out = [(f"demo-{one}", [qb, str(DEMOS / one / "TSC.BAS"), "--dialect", "qb45", "--runtime", "qb45"]) for one in ("qbdemo", "oimad", "deedlines")]
    for pattern, tool in (("tests/run/qb/*.bas", qb), ("bench/*/*.bas", qb), ("bench/parity/*/*.bas", qb), ("bench/*/*.c", cc), ("bench/parity/*/*.c", cc),
                          ("bench/*/*.nib", nib), ("bench/parity/*/*.nib", nib), ("examples/*.nib", nib)):
        out += [(f"{Path(file).parent.name}/{Path(file).name}", [tool, file, *dialect(tool == qb, file), *settings(file)]) for file in sorted(glob.glob(str(ROOT / pattern)))]
    return out


def omf_index(body: bytes, at: int) -> tuple[int, int]:
    """An OMF index and where it ends: one byte, or two where the first has its high bit set."""
    if body[at] & 0x80:
        return ((body[at] & 0x7F) << 8) | body[at + 1], at + 2
    return body[at], at + 1


@dataclass(frozen=True)
class Segment:
    name: str
    klass: str
    size: int
    loaded: int  # bytes an LEDATA or LIDATA record carries; the rest of the segment is uninitialised


# Classes the program does not contribute: the linker's stack, and the debugger's symbol and type records.
NOT_THE_PROGRAMS = {"STACK", "DEBSYM", "DEBTYP"}


def iterated(body: bytes, at: int, wide: bool) -> tuple[int, int]:
    """Bytes an LIDATA block expands to, and where it ends: a repeat count, a block count, then nested blocks or content."""
    repeat = int.from_bytes(body[at : at + (4 if wide else 2)], "little")
    at += 4 if wide else 2
    blocks = int.from_bytes(body[at : at + 2], "little")
    at += 2
    if blocks == 0:
        return repeat * body[at], at + 1 + body[at]
    inner = 0
    for _ in range(blocks):
        size, at = iterated(body, at, wide)
        inner += size
    return repeat * inner, at


def expanded(body: bytes, at: int, wide: bool) -> int:
    """Bytes the data blocks of an LIDATA record expand to."""
    total = 0
    while at < len(body):
        size, at = iterated(body, at, wide)
        total += size
    return total


def segments(obj: Path) -> list[Segment]:
    """The OMF object's segments, with the bytes its data records carry. Only an object: a linked image or a
    library has other records, and COMDAT, which this reader does not follow, is refused rather than missed."""
    data = Path(obj).read_bytes()
    if not data or data[0] != 0x80:
        raise ValueError(f"{obj} is not an OMF object: it does not start with a THEADR record")
    names: list[str] = []
    found: list[list] = []
    at = 0
    while at + 3 <= len(data):
        kind, length = data[at], int.from_bytes(data[at + 1 : at + 3], "little")
        body = data[at + 3 : at + 2 + length]  # without the checksum
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
            name, i = omf_index(body, i)
            klass, i = omf_index(body, i)
            found.append([names[name - 1], names[klass - 1], size, 0])
        elif kind in (0xA0, 0xA1, 0xA2, 0xA3):  # LEDATA, LIDATA
            wide = kind & 1 == 1
            segment, i = omf_index(body, 0)
            i += 4 if wide else 2  # the offset
            found[segment - 1][3] += len(body) - i if kind < 0xA2 else expanded(body, i, wide)
        elif kind in (0xC2, 0xC3):
            raise ValueError(f"{obj} has COMDAT records, which segments() does not read")
    return [Segment(name, klass, size, min(loaded, size)) for name, klass, size, loaded in found]


def is_code(segment: Segment) -> bool:
    return segment.klass.upper().endswith("CODE")  # CODE, and the BASIC runtime's BC_CODE


def code_bytes(obj: Path) -> int:
    """Bytes of the object's code segments (a class named ...CODE): its SEGDEF lengths. The file's size counts
    the fixup, symbol and debug records as well, which the linker consumes and the program lacks."""
    return sum(one.size for one in segments(obj) if is_code(one))


def data_bytes(obj: Path) -> tuple[int, int]:
    """(initialised, uninitialised) bytes of the object's data segments, whatever their class (DATA, CONST, FAR_BSS, BC_VARS,
    BC_SEGS): the bytes its LEDATA and LIDATA records carry, and the rest of each SEGDEF. Not the stack, nor debug records."""
    kept = [one for one in segments(obj) if not is_code(one) and one.klass.upper() not in NOT_THE_PROGRAMS]
    return sum(one.loaded for one in kept), sum(one.size - one.loaded for one in kept)


def measure(command: list[str], level: str) -> tuple[int, int, int] | None:
    with tempfile.TemporaryDirectory() as directory:
        obj = f"{directory}/x.obj"
        done = subprocess.run([*command, level, "-o", obj], capture_output=True, text=True, env={**environ, "LLRM_DEBUG": "cost"}, timeout=300)
        if not Path(obj).exists():
            return None
        found = [tuple(map(int, one)) for one in COST.findall(done.stderr)]
        return code_bytes(Path(obj)), sum(i for i, _ in found), sum(m for _, m in found)


SUB_SP = re.compile(r"^\s*sub sp, (\d+)\s*$", re.M)


def frame(command: list[str], level: str) -> int | None:
    """Bytes the program's procedures reserve below BP: every `sub sp, N` of its listing."""
    with tempfile.TemporaryDirectory() as directory:
        listing = f"{directory}/x.s"
        subprocess.run([*command, level, "-S", "-o", listing], capture_output=True, text=True, timeout=300)
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
