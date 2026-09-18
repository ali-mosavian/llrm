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
    where: "dict | None" = None,
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
    from qbopt.backend import allocate

    confined = allocate.classes(body) if where is not None else {}
    for value in order:
        width = widths.get(value)
        if width is None:
            continue
        if done is not None and value in done:
            continue
        for form in (_regional, _local, _per_block):
            plan = form(cut, value, live, index, deep)
            if plan is None or not _fits(value, plan, live, index, where, confined):
                continue
            carved = _carved(cut, value, _next_value(cut), width, plan)
            if carved is not cut:
                cut = carved
                if done is not None:
                    done.add(value)
                break
    return cut


def loop_bases(body: lir.LirBody, values: frozenset[int]) -> tuple[lir.LirBody, frozenset[int]]:
    """Carve spilled address bases into the natural loops that reuse them.

    An address owner can be inexpensive to recreate in cold code and still
    be the wrong value to recreate for every trip through a loop.  Its normal
    live range may also cross a call after that loop, making a whole-range
    register preference prohibitively expensive.  This is the middle rung:
    copy the owner at the loop entry, use the short value throughout the
    natural loop, and restore the original only where code outside the loop
    still needs it.

    The caller decides whether the fresh pieces actually receive registers;
    this helper only establishes the control-flow-correct split.  Candidates
    are restricted to values which are both memory bases in the loop and
    referenced outside it.  A one-use address or a value confined to the
    loop cannot recover the copy cost, so it belongs to ordinary allocation.
    """
    if not values:
        return body, frozenset()
    from qbopt.analysis import loops

    result = body
    kept: set[int] = set()
    for loop in loops.loops(result.blocks, result.entry):
        inside = loop.body
        references = {value: _references(result, value) for value in values}
        candidates = {
            value
            for value, found in references.items()
            if any(at in inside for at in found)
            and any(at not in inside for at in found)
            and any(
                isinstance(where, ir.Mem)
                and isinstance(where.base, ir.Held)
                and where.base.value == value
                for block in result.blocks
                if block.at in inside
                for one in block.insns
                if one.what is not None
                for where in (*one.what.dests, *one.what.sources)
            )
        }
        for value in sorted(candidates):
            widths = _widths(result)
            width = widths.get(value)
            if width is None:
                continue
            fresh = _next_value(result)
            carved = _carved(result, value, fresh, width, Region(inside, None))
            if carved is result:
                continue
            result = carved
            kept.add(fresh)
    return result, frozenset(kept)


def _fits(value: int, region: "Region", live: dict, index, where: "dict | None", confined: dict) -> bool:
    """Whether some register is free, in the allocation that failed, for the piece `region` carves.

    LLVM's region split is chosen per physical register, against that
    register's interference inside the region. A piece nothing can hold
    spills anyway, with two copies more: PLASMA's cuts into its pixel loop,
    where every register is busy, outweighed the one cut that kept the outer
    counter in a register, and the whole batch was thrown away.
    """
    if where is None or value not in live:
        return True
    from qbopt.backend import target
    from qbopt.backend import allocate

    spans = [ranges.Segment(*index.span[at]) for at in region.blocks if at in index.span]
    piece = tuple(
        sorted(
            (
                ranges.Segment(max(one.start, span.start), min(one.end, span.end))
                for one in live[value].segments
                for span in spans
                if one.overlaps(span)
            ),
            key=lambda one: one.start,
        )
    )
    if not piece:
        return False
    carved = ranges.Interval(value, piece)
    occupants: dict = {}
    for other, register in where.items():
        if other != value and other in live:
            occupants.setdefault(allocate._whole(register), []).append(other)
    return any(
        not any(live[other].overlaps(carved) for other in occupants.get(allocate._whole(register), ()))
        for register in target.order(confined.get(value))
    )


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
    """`tryLocalSplit`: cut the widest same-block gap between references.

    The value may also be live in other blocks.  `_carved` restores the
    original at this region's boundary, so those references are outside the
    fresh piece and cannot make this split unsafe.  The gap still has to be
    wider than a single instruction or the two pieces are live across each
    other's uses anyway and the copy buys nothing.
    """
    found = _references(body, value)
    gaps = [
        (positions[position + 1] - positions[position], at, positions[position + 1])
        for at, positions in found.items()
        for position in range(len(positions) - 1)
    ]
    if not gaps:
        return None
    gap, at, cut = max(gaps)
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
    # A region block also reached from inside the region cannot take the
    # copy in at its top: the inside edge runs it too, and overwrites the
    # piece with the value from before the region -- a loop header's copy
    # undid its latch's increment. Its outside predecessors take it instead,
    # at their end; on their other paths the piece is written and not read.
    predecessors: dict[int, set[int]] = {}
    for block in body.blocks:
        for where in block.succ:
            predecessors.setdefault(where, set()).add(block.at)
    shared = (
        set() if region.starts_at is not None else {at for at in entered if predecessors.get(at, set()) & region.blocks}
    )
    feeding = {where for at in shared for where in predecessors[at] if where not in region.blocks}
    at_of = {block.at: block for block in body.blocks}
    if any(at == body.entry or not predecessors[at] - region.blocks for at in shared) or any(
        not at_of[where].insns for where in feeding
    ):
        return body  # entered with no outside predecessor to hold the copy
    blocks = []
    changed = False
    deferred: list[tuple[int, int, lir.Insn]] = []
    for block in body.blocks:
        if block.at in feeding:
            insns = list(block.insns)
            place = len(insns) - 1 if _terminates(insns[-1]) else len(insns)
            insns.insert(place, _copy(insns[-1], fresh, value, width))
            blocks.append(replace(block, insns=tuple(insns)))
            changed = True
            continue
        if block.at not in region.blocks:
            blocks.append(block)
            continue
        start = region.starts_at if region.starts_at is not None else 0
        outside = tuple(where for where in block.succ if where not in region.blocks)
        leaves = region.starts_at is not None or bool(outside)
        insns = list(block.insns[:start])
        interior = [_renamed(one, {value: fresh}) for one in block.insns[start:]]
        if not interior:
            blocks.append(block)
            continue
        if (block.at in entered and block.at not in shared) or region.starts_at is not None:
            insns.append(_copy(interior[0], fresh, value, width))
            changed = True
        insns.extend(interior)
        # Only where the original is wanted again. A value whose last
        # reference is inside the region has nothing to restore, and the
        # copy back would be a definition nothing reads.
        # A block which both continues around the loop and exits it must not
        # restore the original unconditionally: that puts a store/copy on
        # every trip just to satisfy the one exit.  Split its leaving edges
        # below, where the copy executes only on the way out.  An ordinary
        # one-successor exit keeps the compact in-block form.
        edge_exit = region.starts_at is None and outside and len(outside) < len(block.succ)
        if leaves and (block.at in live_out or region.starts_at is not None) and not edge_exit:
            back = _copy(interior[-1], value, fresh, width)
            place = len(insns) - 1 if _terminates(insns[-1]) else len(insns)
            # After the last use where every way out leaves the region:
            # LLVM's `leaveIntvAfter`. Held to the block's end, the piece
            # was live across what the block defines after it.
            if all(where not in region.blocks for where in block.succ):
                place = min(place, _after_last(insns, fresh))
            insns.insert(place, back)
            changed = True
        elif edge_exit and (block.at in live_out):
            deferred.extend((block.at, where, interior[-1]) for where in outside)
        blocks.append(replace(block, insns=tuple(insns)))
    if not changed and not deferred:
        return body
    # The bridge owns an exit edge, so the original value is restored only
    # for paths that actually leave the region.  It is deliberately a LIR
    # CFG edit: no MIR fact or source byte is involved, and later layout
    # decides its physical fall-through just like every other block.
    by_at = {block.at: block for block in blocks}
    bridges = []
    next_at = max((block.at for block in body.blocks), default=0) + 1
    for source, outside, beside in deferred:
        bridge = next_at
        next_at += 1
        original = by_at[source]
        rewritten = []
        for one in original.insns:
            what = one.what
            if what is not None and what.target == outside:
                one = replace(one, what=replace(what, target=bridge))
            rewritten.append(one)
        by_at[source] = replace(
            original,
            insns=tuple(rewritten),
            succ=tuple(bridge if one == outside else one for one in original.succ),
        )
        back = _copy(beside, value, fresh, width)
        jump = lir.Insn(
            at=beside.at,
            covers=(beside.at, beside.at),
            what=ir.Semantics(ir.Operation.JUMP, "jmp", (), (), outside),
            defines=(),
            uses=(),
            op=beside.op,
        )
        bridges.append(lir.LirBlock(at=bridge, insns=(back, jump), succ=(outside,)))
    return replace(body, blocks=tuple(by_at[block.at] for block in blocks) + tuple(bridges))


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


def _after_last(insns: "list[lir.Insn]", value: int) -> int:
    """The position after the last instruction naming `value`, and after the parallel copy holding it."""
    place = max((position + 1 for position, one in enumerate(insns) if value in (*one.defines, *one.uses)), default=0)
    while 0 < place < len(insns) and insns[place].group is not None and insns[place].group == insns[place - 1].group:
        place += 1
    return place


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
    from qbopt.backend import spiller

    return spiller._renamed(one, rename) if rename else one


def _next_value(body: lir.LirBody) -> int:
    seen = {0}
    for block in body.blocks:
        seen.update(block.arrives)
        for one in block.insns:
            seen.update(one.defines)
            seen.update(one.uses)
    return max(seen) + 1
