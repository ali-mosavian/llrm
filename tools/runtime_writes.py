#!/usr/bin/env python3
"""What the QuickBASIC runtime actually writes, read off a linked image.

    tools/runtime_writes.py build/nbody-zi/B_NBODY.EXE build/nbody-zi/B_NBODY.MAP

`runtime.py`'s contracts say `writes=ANY` for twenty-one routines, and that
is what stops a global being kept in a register across a PRINT -- which is
every program in the suite. The claim is `established`, cited to the
runtime source, and it is true as far as it goes: B$PRINT does write memory.
The question the contract cannot express is *whose*.

This answers it from the linked image, which is on disk and does not need
the runtime source. The map gives every segment's bounds; the image gives
the runtime's code; and each store says which segment it lands in.

Three questions, and the third is the one that matters:

- **Fixed addresses.** A `mov [1234h],ax` through ds names one cell in
  DGROUP for ever. If none of them is in BC_DATA, no runtime routine can
  write a program's variable by naming it.
- **Its own frame.** `[bp-4]` is the routine's locals, and nobody else's.
- **Through a pointer.** `mov [bx],ax` writes wherever bx points, and the
  only way bx points into BC_DATA is if somebody put it there.

An `es:` override is not DGROUP and is excluded: the six references that
first looked like writes into BC_DATA were `mov word [es:7Ch],5D6h` and
`mov [es:7Eh],ds` -- the runtime installing an INT 1Fh vector at 0:7C.
"""

import struct
import sys
from collections import Counter
from pathlib import Path

from iced_x86 import Decoder
from iced_x86 import Formatter
from iced_x86 import FormatterSyntax
from iced_x86 import InstructionInfoFactory
from iced_x86 import OpAccess
from iced_x86 import Mnemonic
from iced_x86 import Register


# The runtime's own code, which the linker puts in DGROUP's data model.
# Anything else in class CODE has a data segment of its own.
DGROUP_CODE = frozenset({"_TEXT", "RTCODE"})

# What writes the stack rather than data.
_STACK = frozenset(
    {Mnemonic.CALL, Mnemonic.PUSH, Mnemonic.PUSHA, Mnemonic.PUSHF, Mnemonic.ENTER, Mnemonic.INT}
)


def segments(text: str) -> list[tuple[str, int, int, str]]:
    """(name, start, end, class) for every segment the map lists."""
    out = []
    for line in text.splitlines():
        parts = line.split()
        if len(parts) < 5 or not parts[0].endswith("H"):
            continue
        try:
            lo, hi = int(parts[0][:-1], 16), int(parts[1][:-1], 16)
        except ValueError:
            continue
        out.append((parts[3], lo, hi + 1, parts[4]))
    return out


def group_origin(text: str, name: str = "DGROUP") -> int | None:
    """Where a group starts, as a linear address."""
    for line in text.splitlines():
        if name in line and ":" in line:
            seg = line.strip().split(":")[0]
            try:
                return int(seg, 16) * 16
            except ValueError:
                return None
    return None


def image_of(exe: bytes) -> bytes:
    """The load image, past the MZ header."""
    return exe[struct.unpack_from("<H", exe, 8)[0] * 16 :]


def measure(exe: Path, mapping: Path) -> int:
    text = mapping.read_text()
    found = segments(text)
    dgroup = group_origin(text)
    if dgroup is None:
        print("  no DGROUP origin in the map")
        return 1
    image = image_of(exe.read_bytes())
    # Only the code whose `ds` is DGROUP. EMULATOR_TEXT keeps its own
    # data in EMULATOR_DATA, class FAR_DATA, so a bare `[94h]` there is an
    # offset into that and not into DGROUP at all -- counting it said the
    # emulator wrote fifteen times into a program's variables.
    code = [(n, lo, hi) for n, lo, hi, cls in found if cls == "CODE" and n in DGROUP_CODE]
    data = [(n, lo, hi) for n, lo, hi, _c in found if lo >= dgroup]

    info = InstructionInfoFactory()
    fmt = Formatter(FormatterSyntax.NASM)
    fixed: Counter = Counter()
    indirect: Counter = Counter()
    suspicious: list[str] = []

    for name, lo, hi in code:
        for insn in Decoder(16, image[lo:hi], ip=lo):
            # A call and a push write the stack. That is not a write to
            # anybody's data and counting it listed every far call in the
            # runtime as touching a program's variables.
            if insn.mnemonic in _STACK:
                continue
            wrote = [
                one
                for one in info.info(insn).used_memory()
                if one.access in (OpAccess.WRITE, OpAccess.READ_WRITE)
            ]
            if not wrote:
                continue
            seg = insn.segment_prefix
            base, index = insn.memory_base, insn.memory_index
            if seg not in (Register.NONE, Register.DS):
                indirect[f"through {Formatter(FormatterSyntax.NASM).format(insn).split(':')[0]}"] += 1
                continue
            if base == Register.NONE and index == Register.NONE:
                at = dgroup + insn.memory_displacement
                where = next((n for n, a, b in data if a <= at < b), "outside DGROUP")
                fixed[where] += 1
                if where == "BC_DATA":
                    suspicious.append(f"{insn.ip:#07x}: {fmt.format(insn)}")
                continue
            if base in (Register.BP, Register.SP):
                indirect["its own frame"] += 1
                continue
            indirect["through a pointer"] += 1

    print(f"  {exe.name}: DGROUP at {dgroup:#x}")
    print("  writes to a fixed address, by segment:")
    for where, n in fixed.most_common():
        print(f"    {n:5}  {where}")
    print("  writes that are not to a fixed address:")
    for where, n in indirect.most_common():
        print(f"    {n:5}  {where}")
    if suspicious:
        print("  and these name a cell in BC_DATA:")
        for one in suspicious:
            print(f"    {one}")
        return 1
    print("\n  No runtime write names a cell in BC_DATA. A program's variable")
    print("  can only be reached through a pointer the program handed over.")
    return 0


def main(argv: list[str]) -> int:
    if len(argv) != 3:
        print(__doc__.splitlines()[2].strip())
        return 2
    return measure(Path(argv[1]), Path(argv[2]))


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
