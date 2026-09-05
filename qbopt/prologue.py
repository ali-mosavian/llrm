"""Reserving the frame the spiller asked for, and giving it back.

LLVM's `PrologEpilogInserter`. `frame.py` hands out slots and says how much
bigger the frame got; this is what makes that space exist -- `sub sp,N` at
the body's entry and `add sp,N` before every return.

**BC's bodies have no prologue of their own to grow.** The runtime sets the
frame up before the body runs (`B$CENP`), and every `[bp-n]` the body uses
is inside what the runtime reserved. So the space a spill slot needs is not
in the frame BC declared, and taking it means lowering sp ourselves -- and
putting it back, because the caller is the runtime and it did not ask for a
different stack.

Refused where the body's exits cannot all be found. A `sub sp` with no
matching `add sp` on some path is not a missed optimisation, it is a
program that returns to the wrong place.
"""

from dataclasses import replace

from iced_x86 import Register

from qbopt import frame as frames
from qbopt import ir
from qbopt import lir
from qbopt.passes import LIRTransform


class Refused(Exception):
    """The frame cannot be grown safely on this body."""


class Prologue(LIRTransform):
    name = "prologue"

    def __init__(self, frame: frames.Frame, calls: dict | None = None) -> None:
        self.frame = frame
        self.calls = calls or {}

    def transform(self, body: lir.LirBody) -> lir.LirBody:
        return reserved(body, self.frame, self.calls)


def reserved(body: lir.LirBody, frame: frames.Frame, calls: dict | None = None) -> lir.LirBody:
    """`body` with sp lowered by what the frame grew, and put back."""
    if not frame.size:
        return body
    leaves = [
        (block, index)
        for block in body.blocks
        for index, one in enumerate(block.insns)
        if one.what is not None and one.what.op is ir.Operation.RETURN
    ]
    if not leaves and not _ends_the_program(body, calls or {}):
        raise Refused(f"{frame.size} bytes of frame are wanted and this body has no return to give them back at")

    entry = next((block for block in body.blocks if block.at == body.entry), None)
    if entry is None or not entry.insns:
        raise Refused("the entry block has no instruction to put the prologue in front of")

    take = _adjust(entry.insns[0], -frame.size)
    give = {(block.at, index): _adjust(block.insns[index], frame.size) for block, index in leaves}
    return replace(
        body,
        blocks=tuple(
            replace(
                block,
                insns=tuple(_woven(block, take if block.at == body.entry else None, give)),
            )
            for block in body.blocks
        ),
    )


def _woven(block: lir.LirBlock, take: "lir.Insn | None", give: dict) -> "list[lir.Insn]":
    """The block with the prologue in front and an epilogue before each return."""
    out: list[lir.Insn] = [take] if take is not None else []
    for index, one in enumerate(block.insns):
        found = give.get((block.at, index))
        if found is not None:
            out.append(found)
        out.append(one)
    return out


def _adjust(beside: lir.Insn, by: int) -> lir.Insn:
    """`sub sp,N` or `add sp,N`, claiming none of the original bytes."""
    at = beside.covers[0] if beside.covers else beside.at
    name = "add" if by > 0 else "sub"
    return lir.Insn(
        at=beside.at,
        covers=(at, at),
        what=ir.Semantics(
            ir.Operation.BINARY,
            name,
            (ir.Reg(Register.SP, 2),),
            (ir.Reg(Register.SP, 2), ir.Imm(abs(by), 2)),
        ),
        defines=(),
        uses=(),
        op=beside.op,
    )


# The runtime call that ends the program. A body whose last instruction is
# this one never returns to anybody, so the stack it leaves behind is not
# read: sp may be lowered and never put back. Every BC main body ends here,
# which is the difference between spilling being available in one and not.
ENDS = "B$CENP"


def _ends_the_program(body: lir.LirBody, calls: dict) -> bool:
    """Whether this body hands control to the runtime's exit and never returns.

    Asked of the whole body rather than of its last instruction: BC pads
    the end of a code segment with zeros, `00 00` decodes as `add [bx+si],al`,
    and reachability walks in -- so the last instruction is routinely not a
    terminator at all. A body that calls the exit and has no return of its
    own leaves by that call on every path.
    """
    return any(calls.get(one.at, "").upper() == ENDS for block in body.blocks for one in block.insns)
