"""Reserving the frame the spiller asked for, and giving it back.

LLVM's `PrologEpilogInserter`. `frame.py` hands out slots and says how much
bigger the frame got; this is what makes that space exist -- `sub sp,N` at
the body's entry and `add sp,N` before every return.

For a runtime-framed procedure, reserve after B$ENRA establishes BP and
release before B$EXSA tears it down. Reserving before entry moves the
arguments relative to the frame; releasing after exit adjusts the caller's
stack. Main bodies already have their frame when their code starts.

Refused where the body's exits cannot all be found. A `sub sp` with no
matching `add sp` on some path is not a missed optimisation, it is a
program that returns to the wrong place.
"""

from dataclasses import replace

from iced_x86 import Register

from qbopt.model import ir
from qbopt.model import lir
from qbopt.backend import frame as frames
from qbopt.model.passes import LIRTransform


class Refused(Exception):
    """The frame cannot be grown safely on this body."""


def _native_anchor(block: lir.LirBlock, at: int, role: str) -> int:
    anchors = [index for index, one in enumerate(block.insns) if one.at == at]
    covered = [
        index
        for index in anchors
        if block.insns[index].covers is not None and block.insns[index].covers[0] < block.insns[index].covers[1]
    ]
    if not anchors or anchors != list(range(anchors[0], anchors[-1] + 1)) or len(covered) != 1:
        raise Refused(f"native frame {role} anchor was lost or duplicated")
    return anchors[0]


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
    if frame.native is not None:
        body = _arguments(body, frame.size)
    calls = calls or {}
    entry = next((block for block in body.blocks if block.at == body.entry), None)
    if entry is None or not entry.insns:
        raise Refused("the entry block has no instruction to put the prologue in front of")
    runtime_entry = next(
        (
            index
            for index, one in enumerate(entry.insns)
            if calls.get(one.at, "").upper() == frames.ENTER and (one.what is None or one.what.op is ir.Operation.CALL)
        ),
        None,
    )
    leaves = [
        (block, index)
        for block in body.blocks
        for index, one in enumerate(block.insns)
        if (calls.get(one.at, "").upper() == frames.LEAVE and (one.what is None or one.what.op is ir.Operation.CALL))
        or (runtime_entry is None and one.what is not None and one.what.op is ir.Operation.RETURN)
    ]
    entry_index = runtime_entry + 1 if runtime_entry is not None else 0
    if frame.native is not None:
        if runtime_entry is not None:
            raise Refused("native and runtime frame plans cannot be combined")
        entry_index = _native_anchor(entry, frame.native.entry.reserve_at, "reservation")
        leaves = [
            (block, _native_anchor(block, at, "release"))
            for at in frame.native.releases
            for block in body.blocks
            if any(one.at == at for one in block.insns)
        ]
        if len(leaves) != len(frame.native.releases):
            raise Refused("native frame release anchor was lost or duplicated")
    if not leaves and not body.noreturn and not _ends_the_program(body, calls or {}):
        raise Refused(f"{frame.size} bytes of frame are wanted and this body has no return to give them back at")

    take = _adjust(entry.insns[0], -frame.size)
    give = {(block.at, index): _adjust(block.insns[index], frame.size) for block, index in leaves}
    return replace(
        body,
        blocks=tuple(
            replace(
                block,
                insns=tuple(
                    _woven(
                        block,
                        take if block.at == body.entry else None,
                        give,
                        entry_index,
                    )
                ),
            )
            for block in body.blocks
        ),
    )


def _arguments(body: lir.LirBody, size: int) -> lir.LirBody:
    def operand(where: ir.Loc) -> ir.Loc:
        if isinstance(where, ir.Mem) and where.stack_argument and where.addr is not None:
            displacement = where.addr.disp - size
            if displacement < -0x8000:
                raise Refused("outgoing argument exceeds the native frame displacement range")
            return replace(where, addr=replace(where.addr, disp=displacement))
        return where

    return replace(
        body,
        blocks=tuple(
            replace(
                block,
                insns=tuple(
                    replace(
                        one,
                        what=replace(
                            one.what,
                            dests=tuple(map(operand, one.what.dests)),
                            sources=tuple(map(operand, one.what.sources)),
                        ),
                    )
                    if one.what is not None
                    else one
                    for one in block.insns
                ),
            )
            for block in body.blocks
        ),
    )


def _woven(block: lir.LirBlock, take: "lir.Insn | None", give: dict, entry_index: int = 0) -> "list[lir.Insn]":
    """The block with the prologue in front and an epilogue before each return."""
    out: list[lir.Insn] = []
    for index, one in enumerate(block.insns):
        if take is not None and index == entry_index:
            out.append(replace(take, at=one.at, covers=(one.at, one.at), op=one.op))
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
        frame_adjust=True,
    )


# The runtime call that ends the program. A body whose last instruction is
# this one never returns to anybody, so the stack it leaves behind is not
# read: sp may be lowered and never put back. Every BC main body ends here,
# which is the difference between spilling being available in one and not.
ENDS = frozenset({"B$CEND", "B$CENP"})


def _ends_the_program(body: lir.LirBody, calls: dict) -> bool:
    """Whether this body hands control to the runtime's exit and never returns.

    Asked of the whole body rather than of its last instruction: BC pads
    the end of a code segment with zeros, `00 00` decodes as `add [bx+si],al`,
    and reachability walks in -- so the last instruction is routinely not a
    terminator at all. A body that calls the exit and has no return of its
    own leaves by that call on every path.
    """
    return any(calls.get(one.at, "").upper() in ENDS for block in body.blocks for one in block.insns)
