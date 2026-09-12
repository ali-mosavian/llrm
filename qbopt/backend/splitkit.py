"""Splitting a live range instead of spilling the whole of it.

LLVM's `SplitKit`, driven by `RegAllocGreedy`'s split ladder. A value the
allocator cannot place anywhere is not always a value that has to live in
memory: cut into pieces, the piece covering the uses that matter may fit
where the whole of it did not, and the copy joining the pieces is cheaper
than the store and the load spilling would cost.

Three forms, tried in LLVM's order and for its reasons:

`_regional` is `tryRegionSplit`. The uses at the deepest loop nesting are
carved into a value of their own, so that piece is short and the complement
-- which crosses the cold blocks between them -- is what spills. The
degenerate case of it is a value that crosses a loop it never touches, and
that was the only form this module had.

`_local` is `tryLocalSplit`. Within one block, the widest gap between
consecutive references is where the range is cut, so neither half is live
across the other's uses and the two can take different registers.

`_per_block` is `tryBlockSplit`, the fallback: every block that references
the value gets its own piece, which is the most a split can do before
spilling is the only answer left.

**How a piece is carved.** A copy in, the interior renamed, and a copy back
before control leaves the region. The copy back is what makes this need no
phi insertion: the original value is correct again on every edge out of the
region, so nothing outside the region is rewritten and the two pieces meet
nowhere. LLVM's `enterIntvBefore` and `leaveIntvAfter`, and the coalescer
is what removes either copy again when both pieces land in one register.
"""

from dataclasses import replace
from dataclasses import dataclass

from qbopt.model import ir
from qbopt.model import lir
from qbopt.model.passes import LIRTransform
from qbopt.analysis import intervals as ranges


class Splitter(LIRTransform):
    name = "split"

    def __init__(self, only: "frozenset[int] | None" = None) -> None:
        self.only = only

    def transform(self, body: lir.LirBody) -> lir.LirBody:
        return split(body, self.only)


def split(
    body: lir.LirBody,
    only: "frozenset[int] | None" = None,
    done: "set[int] | None" = None,
) -> lir.LirBody:
    """`body` with one failing value's range cut, or `body` unchanged.

    Every value the allocator failed on, not one. `RegAllocGreedy` splits
    per failure because its queue is per value; this runs inside a loop
    that re-allocates the whole body each round, so cutting one range a
    round would need a round per spilled value and there are sixteen of
    them against twelve rounds.

    Still only the values that failed. Cutting every range that merely
    crosses a loop -- which is what this did before `only` -- cost 12,329
    bytes over the corpus and freed nothing.

    Widest range first, since that is the one holding a register across
    the most of the body, and the earlier cuts are what the later plans
    are then computed against.

    `done` is LLVM's `RS_Split2` stage, and it is not optional. A cut
    shortens a range, which lowers `weight` -- references over live slots
    -- so re-cutting the same value reduces its *cost* every time while
    never placing it. qbdemo span ten of its twelve rounds that way,
    trimming one spill's cost from 1.86 to 1.68 and spilling it anyway.
    A value is cut once; after that memory is the answer.
    """
    index = ranges.indexed(body)
    live = ranges.intervals(body, index)
    wanted = sorted(only) if only is not None else sorted(live)
    if not wanted:
        return body
    widths = _widths(body)
    deep = ranges.depths(body)
    order = sorted(
        (value for value in wanted if value in live),
        key=lambda value: (-live[value].size, value),
    )
    cut = body
    for value in order:
        width = widths.get(value)
        if width is None:
            continue
        if done is not None and value in done:
            continue
        for form in (_regional, _local, _per_block):
            plan = form(cut, value, live, index, deep)
            if plan is None:
                continue
            carved = _carved(cut, value, _next_value(cut), width, plan)
            if carved is not cut:
                cut = carved
                if done is not None:
                    done.add(value)
                break
    return cut


def _references(body: lir.LirBody, value: int) -> dict[int, list[int]]:
    """Per block, the positions in it that name this value."""
    out: dict[int, list[int]] = {}
    for block in body.blocks:
        found = [position for position, one in enumerate(block.insns) if value in one.defines or value in one.uses]
        if found:
            out[block.at] = found
    return out


def _regional(body, value, live, index, deep) -> "Region | None":
    """`tryRegionSplit`: the deepest-nested blocks that use it, carved out.

    The piece that keeps a register is the one covering the hottest uses.
    Its complement crosses whatever lies between them and is what spills,
    which is the trade the whole ladder exists to make -- memory traffic in
    the cold part rather than in all of it.

    Refused where every reference is already at one depth: there is no
    colder complement to give up, so the cut would only add two copies.
    """
    found = _references(body, value)
    if len(found) < 2:
        return None
    depths = {at: deep.get(at, 0) for at in found}
    hottest = max(depths.values())
    if hottest == 0 or all(one == hottest for one in depths.values()):
        return None
    region = frozenset(at for at, one in depths.items() if one == hottest)
    return Region(region, None)


def _local(body, value, live, index, deep) -> "Region | None":
    """`tryLocalSplit`: one block, cut at the widest gap between references.

    Only for a range living in a single block. The gap has to be wider than
    a single instruction or the two pieces are live across each other's
    uses anyway and the copy buys nothing.
    """
    found = _references(body, value)
    if len(found) != 1:
        return None
    at, positions = next(iter(found.items()))
    if len(positions) < 2:
        return None
    gap, cut = max(
        ((positions[i + 1] - positions[i], positions[i + 1]) for i in range(len(positions) - 1)),
        default=(0, 0),
    )
    if gap < 2:
        return None
    return Region(frozenset({at}), cut)


def _per_block(body, value, live, index, deep) -> "Region | None":
    """`tryBlockSplit`: the single deepest block that references it.

    The last thing to try. One block is the smallest region there is, so
    if this does not fit either then nothing short of memory will, which
    is what the caller does next.
    """
    found = _references(body, value)
    if len(found) < 2:
        return None
    at = max(found, key=lambda one: (deep.get(one, 0), -one))
    if deep.get(at, 0) == 0:
        return None
    return Region(frozenset({at}), None)


@dataclass(frozen=True, slots=True)
class Region:
    """Where a piece lives: a set of blocks, and where inside one it starts.

    `starts_at` is a position in the single block of a local split and None
    for a whole-block region. Keeping both in one shape is what lets
    `_carved` be the only code that rewrites instructions.
    """

    blocks: frozenset[int]
    starts_at: "int | None"


def _carved(body: lir.LirBody, value: int, fresh: int, width: int, region: Region) -> lir.LirBody:
    """`body` with `region` reading a fresh value, joined by copies.

    A copy in wherever control enters the region, and a copy back wherever
    it leaves. The copy back goes before the block's terminator rather than
    at the top of the successor: a successor may be reached from outside
    the region too, and writing the original there would hand it a value
    the other path never computed.
    """
    live_out = _live_out(body, value)
    entered = _entries(body, region)
    blocks = []
    changed = False
    for block in body.blocks:
        if block.at not in region.blocks:
            blocks.append(block)
            continue
        start = region.starts_at if region.starts_at is not None else 0
        leaves = region.starts_at is not None or any(where not in region.blocks for where in block.succ)
        insns = list(block.insns[:start])
        interior = [_renamed(one, {value: fresh}) for one in block.insns[start:]]
        if not interior:
            blocks.append(block)
            continue
        if block.at in entered or region.starts_at is not None:
            insns.append(_copy(interior[0], fresh, value, width))
            changed = True
        insns.extend(interior)
        # Only where the original is wanted again. A value whose last
        # reference is inside the region has nothing to restore, and the
        # copy back would be a definition nothing reads.
        if leaves and (block.at in live_out or region.starts_at is not None):
            back = _copy(interior[-1], value, fresh, width)
            place = len(insns) - 1 if _terminates(insns[-1]) else len(insns)
            insns.insert(place, back)
            changed = True
        blocks.append(replace(block, insns=tuple(insns)))
    return replace(body, blocks=tuple(blocks)) if changed else body


def _entries(body: lir.LirBody, region: Region) -> set[int]:
    """Region blocks control can reach from outside it."""
    inside = region.blocks
    out = {block.at for block in body.blocks if block.at in inside and block.at == body.entry}
    for block in body.blocks:
        if block.at in inside:
            continue
        out.update(where for where in block.succ if where in inside)
    # A region block with no predecessor inside it is entered whatever
    # reaches it: a loop header is reached from its own latch as well.
    reached = {where for block in body.blocks if block.at in inside for where in block.succ}
    out.update(at for at in inside if at not in reached)
    return out


def _live_out(body: lir.LirBody, value: int) -> set[int]:
    """Blocks this value is still wanted after."""
    from qbopt.backend import allocate

    _incoming, outgoing = allocate.live(body)
    return {at for at, values in outgoing.items() if value in values}


def _terminates(one: lir.Insn) -> bool:
    what = one.what
    return what is not None and what.op in (ir.Operation.JUMP, ir.Operation.BRANCH)


def _copy(beside: lir.Insn, into: int, out_of: int, width: int) -> lir.Insn:
    """A move that claims none of the original bytes."""
    at = beside.covers[0] if beside.covers else beside.at
    return lir.Insn(
        at=beside.at,
        covers=(at, at),
        what=ir.Semantics(ir.Operation.MOVE, "mov", (ir.Held(into, width),), (ir.Held(out_of, width),)),
        defines=(into,),
        uses=(out_of,),
        op=beside.op,
    )


def _widths(body: lir.LirBody) -> dict[int, int]:
    """How wide each value is, from the widest operand naming it."""
    out: dict[int, int] = {}
    for one in body.insns:
        operands = (*one.what.dests, *one.what.sources) if one.what is not None else ()
        for operand in operands:
            for held in ir.values(operand):
                out[held.value] = max(out.get(held.value, 0), held.width)
        for held, _register in (*one.requires, *one.delivers):
            out[held.value] = max(out.get(held.value, 0), held.width)
        for named, wide in one.widths:
            out[named] = max(out.get(named, 0), wide)
    return out


def _renamed(one: lir.Insn, rename: dict[int, int]) -> lir.Insn:
    if not rename:
        return one
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
    """One operand with every value it names put through the rename.

    Through `ir.mapped`, so a cell's base is renamed with the rest: this
    looked in `Mem.through`, which holds a register, and a cut past a load
    through a pointer renamed `uses` and left the cell on the value the
    cut had just ended.
    """
    return ir.mapped(where, lambda one: ir.Held(rename.get(one.value, one.value), one.width))


def _next_value(body: lir.LirBody) -> int:
    seen = {0}
    for block in body.blocks:
        seen.update(block.arrives)
        for one in block.insns:
            seen.update(one.defines)
            seen.update(one.uses)
    return max(seen) + 1
