"""Spilling: a value the allocator would not keep, kept in memory instead.

LLVM's `InlineSpiller`. Choosing to spill is the allocator's; making the
program work afterwards is this pass's, and until it existed the choice was
made and nothing acted on it -- `allocate.Spilled` was raised on 237 of 487
objects because an operand still named a value with no register.

Every definition of a spilled value becomes a store into its frame slot,
and every use a load out of it into a fresh value that lives only across
that one instruction. LLVM calls the fresh value the reload's, and it is
what makes the spilled value's live range vanish: nothing is live between
the store and the load, so the register the value wanted is free.

**Not folded.** LLVM tries to fold the reload into the instruction that
reads it -- `add ax,[bp-12h]` rather than a load and then an add -- which
is a peephole over the result and belongs after this rather than inside it.
"""

from dataclasses import replace

from qbopt import frame as frames
from qbopt import ir
from qbopt import lir
from qbopt.passes import LIRTransform


class Spiller(LIRTransform):
    name = "spill"

    def __init__(self, spilled: "frozenset[int]", frame: "frames.Frame | None" = None) -> None:
        self.spilled = spilled
        self.frame = frame

    def transform(self, body: lir.LirBody) -> lir.LirBody:
        return spilled(body, self.spilled, self.frame)


def spilled(body: lir.LirBody, values: "frozenset[int]", frame: "frames.Frame | None" = None) -> lir.LirBody:
    """`body` with each of `values` living in a frame slot."""
    if not values:
        return body
    frame = frame if frame is not None else frames.of(body)
    fresh = _next_value(body)

    blocks = []
    for block in body.blocks:
        insns: list[lir.Insn] = []
        for one in block.insns:
            before, after, rename = [], [], {}
            for value in one.uses:
                if value not in values:
                    continue
                rename[value] = fresh
                before.append(_reload(one, fresh, frame.cell(value, _width(one, value))))
                fresh += 1
            for value in one.defines:
                if value not in values:
                    continue
                rename[value] = fresh
                after.append(_store(one, fresh, frame.cell(value, _width(one, value))))
                fresh += 1
            insns += before
            insns.append(_renamed(one, rename) if rename else one)
            insns += after
        blocks.append(replace(block, insns=tuple(insns)))
    return replace(body, blocks=tuple(blocks))


def _width(one: lir.Insn, value: int) -> int:
    """How wide this instruction reads or writes the value, defaulting to a word."""
    if one.what is None:
        return frames.WORD
    for where in (*one.what.dests, *one.what.sources):
        if isinstance(where, ir.Held) and where.value == value:
            return where.width
    return frames.WORD


def _reload(beside: lir.Insn, into: int, cell: ir.Mem) -> lir.Insn:
    """The load that puts a spilled value back for one instruction."""
    return _inserted(beside, ir.Semantics(ir.Operation.MOVE, "mov", (ir.Held(into, cell.width),), (cell,)), (into,), ())


def _store(beside: lir.Insn, out_of: int, cell: ir.Mem) -> lir.Insn:
    """The store that puts a spilled value away as soon as it is written."""
    return _inserted(beside, ir.Semantics(ir.Operation.MOVE, "mov", (cell,), (ir.Held(out_of, cell.width),)), (), (out_of,))


def _inserted(beside: lir.Insn, what: ir.Semantics, defines: tuple, uses: tuple) -> lir.Insn:
    """An instruction that stands beside another and claims none of its bytes."""
    at = beside.covers[0] if beside.covers else beside.at
    return lir.Insn(at=beside.at, covers=(at, at), what=what, defines=defines, uses=uses, op=beside.op)


def _renamed(one: lir.Insn, rename: dict[int, int]) -> lir.Insn:
    """The instruction reading and writing the reload's value instead."""
    what = one.what
    if what is None:
        return replace(
            one,
            defines=tuple(rename.get(v, v) for v in one.defines),
            uses=tuple(rename.get(v, v) for v in one.uses),
        )
    return replace(
        one,
        what=ir.Semantics(
            what.op,
            what.name,
            tuple(_settled(x, rename) for x in what.dests),
            tuple(_settled(x, rename) for x in what.sources),
            what.target,
        ),
        defines=tuple(rename.get(v, v) for v in one.defines),
        uses=tuple(rename.get(v, v) for v in one.uses),
    )


def _settled(where, rename: dict[int, int]):
    if isinstance(where, ir.Held) and where.value in rename:
        return ir.Held(rename[where.value], where.width)
    if isinstance(where, ir.Mem) and isinstance(where.through, ir.Held) and where.through.value in rename:
        return replace(where, through=ir.Held(rename[where.through.value], where.through.width))
    return where


def _next_value(body: lir.LirBody) -> int:
    """One past the highest value id this body names."""
    seen = {0}
    for block in body.blocks:
        seen.update(block.arrives)
        for one in block.insns:
            seen.update(one.defines)
            seen.update(one.uses)
    return max(seen) + 1
