#!/usr/bin/env python3
"""object_bytes.py TARGET OBJECT: `CODE DATA` bytes of one compiled object, printed. Exit 2 and a message where they cannot be read.
DATA is the initialised, constant and zero-filled data the object carries (an audit row is code + data); CODE alone is size(1)'s `.text`.

Never the file's size: an OMF object's records, a z80 `.s` text and a 6502 `.s` text each held 4-6x the code of tables that read them
(wasm-b's cells.sh fell back to `stat -c %s`: +1052 where the assembled _CODE was +175). A size that cannot be parsed is an error, not a fallback.
"""
import re
import subprocess
import sys
import tempfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent))

import elfsize  # noqa: E402
import sizes  # noqa: E402

Z80_CODE_AREAS = {"_CODE", "_HOME", "_GSINIT", "_GSFINAL", "_INITIALIZER"}
Z80_DATA_AREAS = {"_DATA", "_CONST", "_INITIALIZED"}
ELF_INITIALISED = re.compile(r"\.(data|rodata|sdata|tdata)\b")  # .data.rel.ro, .rodata.cst8 (constant pools), .rodata.str1.1 included
ELF_ZERO_FILLED = re.compile(r"\.(bss|sbss|tbss)\b")


class Unreadable(Exception):
    pass


def run(cmd: list[str], **kwargs) -> subprocess.CompletedProcess:
    try:
        done = subprocess.run(cmd, capture_output=True, text=True, timeout=120, **kwargs)
    except (OSError, subprocess.TimeoutExpired) as error:
        raise Unreadable(f"{cmd[0]}: {error}") from error
    if done.returncode != 0:
        raise Unreadable(f"{cmd[0]} failed: {(done.stderr or done.stdout).strip().splitlines()[:1]}")
    return done


def z80_bytes(source: Path) -> tuple[int, int]:
    """The `.s` llrm-cc writes for z80, assembled by sdasz80: the sizes of the code areas in the `.rel`."""
    with tempfile.TemporaryDirectory() as work:
        run(["sdasz80", "-plosgff", "-o", f"{work}/o.rel", str(source)])
        rows = [re.match(r"A (\S+) size ([0-9A-Fa-f]+) flags", line) for line in open(f"{work}/o.rel")]
    sizes_by_area = [(m.group(1), int(m.group(2), 16)) for m in rows if m]
    if not any(area in Z80_CODE_AREAS for area, _ in sizes_by_area):
        raise Unreadable(f"{source}: the assembled object has no code area")
    return sum(size for area, size in sizes_by_area if area in Z80_CODE_AREAS), sum(size for area, size in sizes_by_area if area in Z80_DATA_AREAS)


def mos6502_bytes(source: Path) -> tuple[int, int]:
    """The `.s` llrm-cc writes for 6502, assembled by ca65: the CODE segment od65 reports."""
    with tempfile.TemporaryDirectory() as work:
        Path(work, "o.s").write_text(".autoimport +\n" + source.read_text())
        run(["ca65", "-t", "sim6502", f"{work}/o.s", "-o", f"{work}/o.o"])
        listing = run(["od65", "--dump-segsize", f"{work}/o.o"]).stdout
    found = re.search(r"CODE:\s+(\d+)", listing)
    if not found:
        raise Unreadable(f"{source}: od65 reports no CODE segment")
    return int(found.group(1)), sum(int(m.group(2)) for m in re.finditer(r"(RODATA|DATA|BSS):\s+(\d+)", listing))


def elf_footprint(obj: Path) -> tuple[int, int, int]:
    """`(code, data, bss)` of an ELF object, read by size(1) (m68k is big-endian; elfsize.code_bytes reads little-endian only): `.text*` is the code,
    the initialised and constant sections (jump tables, constant pools, strings) the data, the zero-filled ones the bss. Not debug, not .comment, not unwind tables."""
    listing = run(["size", "-A", str(obj)]).stdout
    rows = [line.split() for line in listing.splitlines()]
    sized = [(row[0], int(row[1])) for row in rows if len(row) > 1 and row[1].isdigit()]
    return (
        elfsize.text_bytes(listing),
        sum(size for name, size in sized if ELF_INITIALISED.match(name)),
        sum(size for name, size in sized if ELF_ZERO_FILLED.match(name)),
    )


def elf_bytes(obj: Path) -> tuple[int, int]:
    code, data, bss = elf_footprint(obj)
    return code, data + bss


def object_bytes(target: str, obj: Path) -> tuple[int, int]:
    if not obj.is_file() or obj.stat().st_size == 0:
        raise Unreadable(f"{obj}: no object")
    if target.startswith("z80"):
        return z80_bytes(obj)
    if target.startswith("6502"):
        return mos6502_bytes(obj)
    if target.startswith("wasm"):
        raise Unreadable("wasm objects are not read: a module's code section is not an object's .text")
    if sizes.is_elf(obj):
        return elf_bytes(obj)
    if target.startswith("x86_16"):
        return sizes.code_bytes(obj), sum(sizes.data_bytes(obj))
    raise Unreadable(f"{target}: no reader for {obj}")


def main(argv: list[str]) -> int:
    if len(argv) != 3:
        print("usage: object_bytes.py TARGET OBJECT", file=sys.stderr)
        return 2
    try:
        print(*object_bytes(argv[1], Path(argv[2])))
    except Exception as error:  # any failure to read is an error exit, never a number
        print(f"object_bytes: {argv[1]} {argv[2]}: {error}", file=sys.stderr)
        return 2
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
