#!/usr/bin/env python3
"""m32cells.py BINDIR LEVEL [NAME...]: real-flow bytes at m32, ours (llrm-cc) against the pin's clang, per tests/run/c and bench program.

Size rule (as m16, #8466): a cell is padding-free code + data (.data, .rodata*: constant pools, jump tables, strings), strict. Static = data + bss is a second
axis; bss alone is never judged. A constant kept out of .text counts for every column.

Columns: ours; pin at OUR stack alignment (the target description's, via pin.stack_alignment); pin at 16 (the Linux default), labelled CF6.
A row whose pin and ours stack alignments differ is refused unless the pin column carries the CF6 label: the unlabelled pin column was
read at 16 against our 4 and counted cells (select_of_constants, wc_mixed) that a matched pin does not have.
"""

import os
import re
import subprocess
import sys
import tempfile
from concurrent.futures import ThreadPoolExecutor
from dataclasses import dataclass
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parent / "ab"))
import object_bytes  # noqa: E402
import pin  # noqa: E402

ROOT = HERE.parents[1]
PAD = re.compile(r"^(?:(?:data16|cs|ds) )*nop\b|^xchg\s+ax,ax$|^mov\s+(\w+),\1$|^lea\s+(\w+),\[\2(?:\+eiz\*1)?\+0x0\]$|^\.byte")


class Refused(Exception):
    """A row whose two sides were built under different ABI settings, with no label saying so."""


@dataclass(frozen=True)
class Column:
    name: str
    stack_alignment: int
    label: str = ""


def check_row(ours: Column, other: Column) -> None:
    """Refuses `other` unless it is built at ours' stack alignment or labelled CF6."""
    if other.stack_alignment != ours.stack_alignment and other.label != "CF6":
        raise Refused(f"{other.name}: stack alignment {other.stack_alignment} against ours {ours.stack_alignment}, no CF6 label")


def columns() -> tuple[Column, Column, Column]:
    align = pin.stack_alignment(pin.target("m32"))
    return Column("ours", align), Column("pin", align), Column("pin16", 16, "CF6")


def pin_flags(column: Column) -> list[str]:
    """The CF6 column is the pin as clang ships it (no flag: 16, and `stackrealign` on main); the other is told ours."""
    told = [] if column.label == "CF6" else [f"-mstack-alignment={column.stack_alignment}"]
    return ["-m32", "-march=i486", "-fno-pic", "-fno-stack-protector", *told]


def code_bytes(obj: Path) -> int:
    """.text bytes, alignment padding excluded."""
    out = subprocess.run(["objdump", "-d", "-M", "intel", "-w", str(obj)], capture_output=True, text=True, check=True).stdout
    total = 0
    for line in out.splitlines():
        m = re.match(r"\s*[0-9a-f]+:\s+((?:[0-9a-f]{2} )+)\s*(.*)", line)
        if m and m.group(2).strip() and not PAD.match(m.group(2).strip()):
            total += len(m.group(1).split())
    return total


@dataclass(frozen=True)
class Footprint:
    code: int  # padding-free .text
    data: int  # initialised and constant sections
    bss: int

    @property
    def size(self) -> int:
        return self.code + self.data

    @property
    def static(self) -> int:
        return self.data + self.bss


def footprint(obj: Path) -> Footprint:
    _, data, bss = object_bytes.elf_footprint(obj)
    return Footprint(code_bytes(obj), data, bss)


def worse(ours: Footprint, other: Footprint) -> list[str]:
    """The axes on which ours is larger than `other`: size (code + data) and static (data + bss), each strict; bss alone is not an axis."""
    return [f"{axis} {getattr(ours, axis)} vs {getattr(other, axis)}" for axis in ("size", "static") if getattr(ours, axis) > getattr(other, axis)]


def one(src: Path, bindir: Path, level: str, cols: tuple[Column, Column, Column]) -> tuple:
    flag = "-Os" if level == "Os" else "-" + level
    inc = ["-I" + str(ROOT / "bench"), "-I" + str(ROOT / "runtime/shared/posix")]
    ours, *pins = cols
    for column in pins:
        check_row(ours, column)
    clang = subprocess.run([sys.executable, str(ROOT / "tools/pin_clang.py")], capture_output=True, text=True, check=True).stdout.strip()
    name = src.parent.name + "/" + src.stem
    with tempfile.TemporaryDirectory(prefix="m32c-") as t:
        t = Path(t)
        env = dict(os.environ, LLRM_CC_TARGET="x86_32-linux", LLRM_LLC=str(bindir / "llrm-llc"), LLRM_PIN_CLANG=clang)
        try:
            subprocess.run([str(bindir / "llrm-cc"), flag, "-c", "-o", str(t / "o.o"), str(src), *inc], cwd=ROOT, env=env, check=True, capture_output=True)
            sizes = [footprint(t / "o.o")]
            for column in pins:
                subprocess.run([clang, flag, "-c", *pin_flags(column), *inc, str(src), "-o", str(t / f"{column.name}.o")], check=True, capture_output=True)
                sizes.append(footprint(t / f"{column.name}.o"))
        except subprocess.CalledProcessError as e:
            return name, None, (e.stderr or b"")[:60]
    return name, sizes, ""


def main(argv: list[str]) -> int:
    bindir, level, names = Path(argv[0]), argv[1], argv[2:]
    cols = columns()
    srcs = sorted((ROOT / "tests/run/c").glob("*.c")) + sorted((ROOT / "bench").glob("*/*.c"))
    srcs = [s for s in srcs if not names or s.stem in names]
    with ThreadPoolExecutor(int(os.environ.get("JOBS", "6"))) as ex:
        rows = list(ex.map(lambda s: one(s, bindir, level, cols), srcs))
    print("# size = padding-free code + data (.data, .rodata*); static = data + bss, judged apart; bss alone is not judged")
    print("\t".join(["cell", "ours", f"pin(a{cols[1].stack_alignment})", "ours-pin", f"pin16({cols[2].label})", "ours-pin16", "static ours-pin", "static ours-pin16"]))
    failed = 0
    for name, sizes, why in rows:
        if sizes is None:
            failed += 1
            print(f"{name}\tFAIL\t{why}")
        else:
            o, p, p16 = sizes
            print(f"{name}\t{o.size}\t{p.size}\t{o.size - p.size}\t{p16.size}\t{o.size - p16.size}\t{o.static - p.static}\t{o.static - p16.static}")
    print(f"compiled {len(rows) - failed} of {len(rows)}", file=sys.stderr)
    return 1 if failed == len(rows) else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
