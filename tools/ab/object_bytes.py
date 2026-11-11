#!/usr/bin/env python3
"""object_bytes.py TARGET OBJECT: `CODE DATA ALIGN` bytes of one compiled object, printed. Exit 2 and a message where they cannot be read.
DATA is the initialised, constant and zero-filled data the object carries (an audit row is code + data). ALIGN is the alignment padding CODE leaves out.
CODE is padding-free for x86 (x86_32 and x86_64 ELF through elfsize.code_size with the x86 decoder, the OMF through sizes.code_size), ALIGN its padding. Every other ELF target
(arm64, m68k: their padding hides in the section just as well; m68k is big-endian and elfsize reads little-endian only) keeps size(1)'s `.text*` section size WITH padding and prints ALIGN 0: its CODE is not comparable
with an x86 CODE, whose padding is left out.

Never the file's size: an OMF object's records, a z80 `.s` text and a 6502 `.s` text each held 4-6x the code of tables that read them
(wasm-b's cells.sh fell back to `stat -c %s`: +1052 where the assembled _CODE was +175). A size that cannot be parsed is an error, not a fallback.
"""
import re
import subprocess
import sys
import tempfile
from dataclasses import dataclass
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent))

import elfsize  # noqa: E402

sys.path.insert(0, str(HERE.parent.parent / "crates/target/llrm-x86/bench"))

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


def mos6502_bytes(source: Path, cpu: str) -> tuple[int, int]:
    """The `.s` llrm-cc writes for 6502 or 65c02 (`cpu`: ca65's name for it, 65C02 takes `stz`), assembled by ca65: the CODE segment od65 reports."""
    with tempfile.TemporaryDirectory() as work:
        Path(work, "o.s").write_text(".autoimport +\n" + source.read_text())
        run(["ca65", "-t", "sim6502", "--cpu", cpu, f"{work}/o.s", "-o", f"{work}/o.o"])
        listing = run(["od65", "--dump-segsize", f"{work}/o.o"]).stdout
    found = re.search(r"CODE:\s+(\d+)", listing)
    if not found:
        raise Unreadable(f"{source}: od65 reports no CODE segment")
    return int(found.group(1)), sum(int(m.group(2)) for m in re.finditer(r"(RODATA|DATA|BSS):\s+(\d+)", listing))


def wasm_bytes(obj: Path) -> tuple[int, int]:
    """wasm-objdump's Code and Data section sizes of the module llrm-cc writes."""
    listing = run(["wasm-objdump", "-h", str(obj)]).stdout
    sizes_by_section = {m.group(1): int(m.group(2), 16) for m in re.finditer(r"^\s*(\w+) start=\S+ end=\S+ \(size=0x([0-9a-f]+)\)", listing, re.M)}
    if "Code" not in sizes_by_section:
        raise Unreadable(f"{obj}: no Code section")
    return sizes_by_section["Code"], sizes_by_section.get("Data", 0)


@dataclass(frozen=True)
class Footprint:
    code: int  # `.text*`, without alignment padding where the target's reader can tell it (x86), else the section size
    data: int  # initialised and constant sections (jump tables, constant pools, strings)
    bss: int  # zero-filled sections
    align: int = 0  # the padding `code` leaves out; 0 for a target whose reader keeps it in `code`


def elf_footprint(target: str, obj: Path) -> Footprint:
    """x86 ELF: elfsize.code_size and elfsize.data_bytes, the readers sizes.py wraps for the bench (padding-free code; data = allocated non-executable sections but
    `.eh_frame` and `.note*`). Other ELF: size(1) reads every endianness: `.text*` is the code, `.data`/`.rodata*` the data, `.bss` the bss."""
    if target.startswith(("x86_32", "x86_64")):
        from padding import padding  # the x86 decoder (iced_x86), imported where an x86 ELF is read: no other target needs it

        code, align, _ = elfsize.code_size(obj, padding)
        data, bss = elfsize.data_bytes(obj)
        return Footprint(code, data, bss, align)
    listing = run(["size", "-A", str(obj)]).stdout
    rows = [line.split() for line in listing.splitlines()]
    sized = [(row[0], int(row[1])) for row in rows if len(row) > 1 and row[1].isdigit()]
    return Footprint(
        elfsize.text_bytes(listing),
        sum(size for name, size in sized if ELF_INITIALISED.match(name)),
        sum(size for name, size in sized if ELF_ZERO_FILLED.match(name)),
    )


def footprint(target: str, obj: Path) -> Footprint:
    if not obj.is_file() or obj.stat().st_size == 0:
        raise Unreadable(f"{obj}: no object")
    if target.startswith("z80"):
        code, data = z80_bytes(obj)
        return Footprint(code, data, 0)
    if target.startswith("6502"):
        code, data = mos6502_bytes(obj, "6502")
        return Footprint(code, data, 0)
    if target.startswith("65c02"):
        code, data = mos6502_bytes(obj, "65C02")
        return Footprint(code, data, 0)
    if target.startswith("wasm"):
        code, data = wasm_bytes(obj)
        return Footprint(code, data, 0)
    if obj.read_bytes()[:4] == b"\x7fELF":
        return elf_footprint(target, obj)
    if target.startswith(("x86_16", "x86_32")):
        import sizes  # the OMF reader, which imports the compiler tooling: only an OMF object (16- or 32-bit Watcom) needs it

        size = sizes.code_size(obj)
        initialised, zero = sizes.data_bytes(obj)
        return Footprint(size.code, initialised, zero, size.align)
    raise Unreadable(f"{target}: no reader for {obj}")


def object_bytes(target: str, obj: Path) -> tuple[int, int]:
    """`(code, data + bss)`, the pair the audit rows use."""
    found = footprint(target, obj)
    return found.code, found.data + found.bss


def main(argv: list[str]) -> int:
    if len(argv) != 3:
        print("usage: object_bytes.py TARGET OBJECT", file=sys.stderr)
        return 2
    try:
        found = footprint(argv[1], Path(argv[2]))
        print(found.code, found.data + found.bss, found.align)
    except Exception as error:  # any failure to read is an error exit, never a number
        print(f"object_bytes: {argv[1]} {argv[2]}: {error}", file=sys.stderr)
        return 2
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
