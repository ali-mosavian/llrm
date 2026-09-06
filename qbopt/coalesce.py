"""Coalescing: a copy whose two values can share a register is not a copy.

Phi elimination and the two-address fixup both work by inserting moves, and
most of them are moves between values that never need to be apart. Where
the source and the destination do not interfere, they can be one value --
and then the move writes a register from itself and goes.

LLVM's `RegisterCoalescer`, and it runs where LLVM runs it: after the two
passes that make the copies, before allocation, so the allocator sees the
merged values rather than pairs it has to hope land in the same register.

Conservative on purpose. LLVM's coalescer proves a great deal more -- it
joins across subregisters, rematerialises, and undoes a join that turns out
to have made the interval uncolourable. This joins a copy only where the
two intervals do not overlap at all and neither is pinned, which is the
half that is always safe.
"""

from dataclasses import replace

from qbopt import intervals as ranges
from qbopt import ir
from qbopt import lir
from qbopt.passes import LIRTransform


class Coalescer(LIRTransform):
    name = "coalesce"

    def __init__(self, pinned: dict | None = None) -> None:
        self.pinned = pinned or {}

    def transform(self, body: lir.LirBody) -> lir.LirBody:
        return joined(body, self.pinned)


def joined(body: lir.LirBody, pinned: dict | None = None) -> lir.LirBody:
    """`body` with every copy this can prove unnecessary removed."""
    pinned = pinned or {}
    live = ranges.intervals(body)
    swap: dict[int, int] = {}
    gone: set[int] = set()

    for block in body.blocks:
        for one in block.insns:
            pair = _copy(one)
            if pair is None:
                continue
            into, out_of = (swap.get(x, x) for x in pair)
            if into == out_of:
                gone.add(id(one))
                continue
            if into in pinned or out_of in pinned:
                continue
            mine, theirs = live.get(into), live.get(out_of)
            if mine is None or theirs is None or mine.overlaps(theirs):
                continue
            # The merged range, not the two that went into it. A value can
            # be the destination of several copies -- lngmix joins v3 with
            # v9 and then, through the rename, v9 with v20 -- and checking
            # each against the interval it started with says both are safe
            # while their union is live across everything between. LLVM's
            # RegisterCoalescer joins the live intervals as it goes so the
            # next join sees what the last one made.
            live[out_of] = _merged(mine, theirs)
            live[into] = live[out_of]
            swap[into] = out_of
            gone.add(id(one))

    if not gone:
        return body
    return replace(
        body,
        blocks=tuple(
            replace(
                block,
                insns=tuple(_renamed(one, swap) for one in block.insns if id(one) not in gone),
                phis=tuple(
                    lir.Phi(swap.get(phi.result, phi.result), tuple((at, swap.get(v, v)) for at, v in phi.incoming))
                    for phi in block.phis
                ),
            )
            for block in body.blocks
        ),
    )


def _merged(one: "ranges.Interval", other: "ranges.Interval") -> "ranges.Interval":
    """One interval covering both, which is what the joined value occupies."""
    runs = sorted((*one.segments, *other.segments), key=lambda x: (x.start, x.end))
    out = [runs[0]]
    for seg in runs[1:]:
        if seg.start <= out[-1].end:
            out[-1] = ranges.Segment(out[-1].start, max(out[-1].end, seg.end))
            continue
        out.append(seg)
    return replace(one, segments=tuple(out), weight=max(one.weight, other.weight))


def _copy(one: lir.Insn) -> "tuple[int, int] | None":
    """The (written, read) pair this instruction is a plain move of."""
    what = one.what
    if what is None or what.op is not ir.Operation.MOVE:
        return None
    if len(what.dests) != 1 or len(what.sources) != 1:
        return None
    into, out_of = what.dests[0], what.sources[0]
    if not isinstance(into, ir.Held) or not isinstance(out_of, ir.Held):
        return None
    return into.value, out_of.value


def _renamed(one: lir.Insn, swap: dict[int, int]) -> lir.Insn:
    """One instruction with every joined value naming its survivor."""
    if one.what is None:
        return replace(
            one,
            defines=tuple(swap.get(v, v) for v in one.defines),
            uses=tuple(swap.get(v, v) for v in one.uses),
        )
    what = one.what
    return replace(
        one,
        what=ir.Semantics(
            what.op,
            what.name,
            tuple(_settled(x, swap) for x in what.dests),
            tuple(_settled(x, swap) for x in what.sources),
            what.target,
        ),
        defines=tuple(swap.get(v, v) for v in one.defines),
        uses=tuple(swap.get(v, v) for v in one.uses),
    )


def _settled(where, swap: dict[int, int]):
    if isinstance(where, ir.Held) and where.value in swap:
        return ir.Held(swap[where.value], where.width)
    return where
