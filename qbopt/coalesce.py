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
to have made the interval uncolourable. This has no undo, so it asks
Briggs before the join instead: a class whose merged form has fewer than K
neighbours of significant degree is still colourable, and one that does not
is refused rather than joined and regretted. divmod-p-g2's 244th legal
join merged a class spanning the whole body -- 33 segments, 16 neighbours
against K = 6 -- and the allocator could place nothing afterwards.

A class, not a chain. Renaming a step at a time is consistent only while
no value is both a key and a value of the map, and a phi's result is
written by one copy per predecessor: bools-p-evt joined v2 with v63 and
then v63 with v61, so a read of v2 became v63 while the only definition of
v63 became v61.
"""

from dataclasses import replace

from qbopt import ir
from qbopt import lir
from qbopt import target
from qbopt import intervals as ranges
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
    from qbopt import allocate

    where_of = allocate.classes(body)
    everything = frozenset(target.AVAILABLE)
    may: dict[int, frozenset] = {one: where_of.get(one, everything) for one in live}
    held: dict[int, int] = dict(pinned)
    near = _adjacent(live)
    parent: dict[int, int] = {}

    def find(one: int) -> int:
        root = one
        while parent.get(root, root) != root:
            root = parent[root]
        while parent.get(one, one) != one:
            one, parent[one] = parent[one], root
        return root

    for block in body.blocks:
        for one in block.insns:
            pair = _copy(one)
            if pair is None:
                continue
            here, there = find(pair[0]), find(pair[1])
            if here == there:
                continue
            # Two pinned to different registers are two registers.
            mine_pin, theirs_pin = held.get(here), held.get(there)
            if mine_pin is not None and theirs_pin is not None and mine_pin != theirs_pin:
                continue
            mine, theirs = live.get(here), live.get(there)
            if mine is None or theirs is None or mine.overlaps(theirs):
                continue
            # The registers the merged class could take, which is what K
            # counts: a value some instruction reaches a cell through is
            # confined to the addressing class, and a class holding one of
            # those is confined with it.
            allowed = may.get(here, everything) & may.get(there, everything)
            if not allowed:
                continue
            neighbours = (near.get(here, set()) | near.get(there, set())) - {here, there}
            k = len(allowed)
            if len([o for o in neighbours if len(near.get(o, ())) >= k]) >= k:
                continue  # Briggs: the merged class would not be colourable
            parent[here] = there
            live[there] = _merged(mine, theirs)
            live.pop(here, None)
            may[there] = allowed
            may.pop(here, None)
            if mine_pin is not None or theirs_pin is not None:
                held[there] = mine_pin if mine_pin is not None else theirs_pin
            held.pop(here, None)
            for other in near.pop(here, set()):
                near.get(other, set()).discard(here)
                if other != there:
                    near.setdefault(other, set()).add(there)
                    neighbours.add(other)
            near[there] = neighbours

    # No early return where nothing joined. A copy whose two ends were
    # already one value is an identity however it got that way, and
    # cleaning it up must not depend on some unrelated pair elsewhere in
    # the body having coalesced -- nor may the bytes it stood for go with
    # it. `_kept` answers both, and with an empty map it only removes what
    # was already an identity.
    swap = {one: find(one) for block in body.blocks for insn in block.insns for one in (*insn.defines, *insn.uses)}
    swap.update({one: find(one) for one in parent})
    return replace(
        body,
        blocks=tuple(
            replace(
                block,
                # Only a copy whose two ends are now one value: everything
                # is named by its class first, and what is left of a joined
                # copy reads a register into itself. `_kept` also hands on
                # the bytes such a copy stood for.
                insns=tuple(_kept(block, swap)),
                phis=tuple(
                    lir.Phi(swap.get(phi.result, phi.result), tuple((at, swap.get(v, v)) for at, v in phi.incoming))
                    for phi in block.phis
                ),
            )
            for block in body.blocks
        ),
    )


def _adjacent(live: dict) -> dict[int, set]:
    """Which values are ever live at the same moment, by value.

    The interference graph Briggs counts degrees in. Bounded first: two
    whose whole spans do not touch cannot have segments that do, and that
    is most pairs.
    """
    bounds = {one: (iv.segments[0].start, iv.segments[-1].end) for one, iv in live.items() if iv.segments}
    out: dict[int, set] = {one: set() for one in live}
    order = sorted(bounds, key=lambda one: bounds[one])
    for index, one in enumerate(order):
        _lo, hi = bounds[one]
        for other in order[index + 1 :]:
            if bounds[other][0] >= hi:
                break
            if live[one].overlaps(live[other]):
                out[one].add(other)
                out[other].add(one)
    return out


def _kept(block: "lir.LirBlock", swap: dict) -> "list[lir.Insn]":
    """One block's instructions, with a joined copy's bytes given away.

    Which copies are identities is this phase's question -- both ends
    resolving to one value -- and `lir.without` is what happens to the
    bytes.
    """

    def identity(one: "lir.Insn") -> bool:
        pair = _copy(one)
        return pair is not None and pair[0] == pair[1]

    return lir.without(block.insns, identity, lambda one: _renamed(one, swap))


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
