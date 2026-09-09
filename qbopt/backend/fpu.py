"""
The emulator's interrupt, replaced by the instruction it stands for.

Under /FPi -- which is every configuration this project has an object for --
BC emits no x87 instruction at all. It emits Open Watcom's emulator
interrupts, and the real opcode lives inside them:

    cd 35 46 c8     int 35h, operand inline   ->   d9 46 c8   fld dword [bp-38h]

declen.py decodes these; wrapped() restores the protocol after selecting
new operands. Turning an emulated site into a native instruction instead
changes what the program needs to run, from any 8086 to one with a
coprocessor. That separate conversion remains opt-in.

Where it is asked for the win is not the byte -- though every site does
shrink by one. It is the interrupt: each of these traps into the emulator,
which decodes the inline operand and does the arithmetic in software. On a
machine with a 387 the same work is one instruction.

**int 3Ch is refused, and has to be.** It stands in for a segment override
whose segment the emulator patches at run time, so the object does not say
which one it is -- declen.py decodes the site as though there were no
override at all (its own docstring says so). Emitting a native instruction
there would mean choosing a segment on no evidence. In qb-qrender that is
201 sites of 2,130; the other 1,929 convert.
"""

from typing import TYPE_CHECKING

from qbopt.frontend.declen import ESC
from qbopt.frontend.declen import Insn
from qbopt.frontend.declen import Stands
from qbopt.frontend.declen import EMULATED
from qbopt.frontend.declen import INTERRUPT
from qbopt.frontend.declen import stood_in_for

if TYPE_CHECKING:
    from qbopt.backend.select import Emitted


def wrapped(made: "Emitted", protocol: int) -> "Emitted | None":
    """Reapply an existing emulator protocol to newly selected x87 bytes."""
    from dataclasses import replace

    code = made.code
    if not code:
        return None
    if protocol == Stands.FWAIT:
        if code != bytes([0x9B]):
            return None
        prefix, tail, shift = bytes([INTERRUPT, Stands.FWAIT]), b"", 1
    elif protocol == Stands.SEGMENTED:
        if code[0] not in ESC:
            return None
        prefix, tail, shift = bytes([INTERRUPT, Stands.SEGMENTED]), code, 2
    elif protocol in EMULATED and code[0] in ESC:
        prefix, tail, shift = bytes([INTERRUPT, EMULATED.start + code[0] - ESC.start]), code[1:], 1
    else:
        return None
    return replace(made, code=prefix + tail,
                   displacement_at=None if made.displacement_at is None else made.displacement_at + shift,
                   immediate_at=None if made.immediate_at is None else made.immediate_at + shift,
                   fields=tuple(field + shift for field in made.fields))


def emulated_at(code: bytes, at: int) -> bool:
    """Whether `at` is an emulator site at all."""
    return code[at : at + 1] == bytes([INTERRUPT]) and len(code) > at + 1


def native(code: bytes, insn: Insn) -> bytes | None:
    """The real instruction this emulated site stands for, or None.

    None where the site is not emulated, where it is the segment-override
    form (see the module docstring), or where the bytes do not decode back
    to the same instruction -- which is the gate, not a formality: this is
    the one rewrite in the pass that changes the machine the output needs,
    so it emits nothing it cannot read back.
    """
    at = insn.at
    if not emulated_at(code, at) or code[at + 1] not in EMULATED and code[at + 1] != Stands.FWAIT:
        return None
    if code[at + 1] == Stands.SEGMENTED:
        return None

    found = stood_in_for(code, at)
    if found is None:
        return None
    stood_in, hidden = found
    if hidden != 1:
        return None

    # The decoded instruction's own length, out of the buffer stood_in_for
    # built -- which carries up to fifteen trailing bytes so iced can decode,
    # and most of them belong to whatever follows.
    wanted = insn.length - 2 + hidden
    out = stood_in[:wanted]
    if len(out) != wanted or not out or out[0] not in ESC and out[0] != 0x9B:
        return None
    return out
