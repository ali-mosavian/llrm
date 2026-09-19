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

from qbopt.model import ir
from qbopt.model import lir
from qbopt.backend import target
from qbopt.model.passes import LIRTransform
from qbopt.analysis import intervals as ranges


class Coalescer(LIRTransform):
    name = "coalesce"

    def __init__(self, pinned: dict | None = None) -> None:
        self.pinned = pinned or {}

    def transform(self, body: lir.LirBody) -> lir.LirBody:
        return joined(body, self.pinned)


def joined(body: lir.LirBody, pinned: dict | None = None) -> lir.LirBody:
    """`body` with every copy this can prove unnecessary removed."""
    pinned = {
        getattr(value, "id", value): ir.ROOT.get(register, register)
        for value, register in {**body.pins, **(pinned or {})}.items()
    }
    from qbopt.backend import allocate

    index = ranges.indexed(body)
    live = ranges.intervals(body, index)
    masks = allocate._masks(body, index)
    widths = allocate._widest(body)
    where_of = allocate.classes(body)
    everything = frozenset(target.AVAILABLE)
    may: dict[int, frozenset] = {one: frozenset(target.order(where_of.get(one))) for one in live}
    for value, register in pinned.items():
        if register not in everything and value not in where_of:
            may[value] = frozenset({register})
    held: dict[int, int] = dict(pinned)
    near = _interference(body)
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
            if mine is None or theirs is None or there in near.get(here, set()):
                continue
            # The registers the merged class could take, which is what K
            # counts: a value some instruction reaches a cell through is
            # confined to the addressing class, and a class holding one of
            # those is confined with it.
            allowed = may.get(here, everything) & may.get(there, everything)
            if not allowed:
                continue
            # A legal copy join can still create an impossible lifetime.  In
            # LOOP the literal zero used by B$ENRA was joined to the loop
            # phi's post-call seed.  The merged value crossed a call that
            # destroys every GPR, so allocation stored it through BP before
            # B$ENRA had established the frame.  Register masks constrain the
            # merged interval just as they constrain allocation: refuse a join
            # with no surviving register, and let the copy rematerialize or
            # occupy a fresh post-call range instead.
            merged = _merged(mine, theirs)
            width = max(widths.get(here, 0), widths.get(there, 0), 1)
            allowed = frozenset(
                register for register in allowed if not allocate._clobbered(merged, register, masks, width)
            )
            if not allowed:
                continue
            if any(pin is not None and pin not in allowed for pin in (mine_pin, theirs_pin)):
                continue
            neighbours = (near.get(here, set()) | near.get(there, set())) - {here, there}
            k = len(allowed)
            # An unpinned neighbour can be coloured last if its degree is
            # smaller than its own palette, not this merged class's palette.
            # Keep the established test around pins, where recolouring is
            # not free and the heterogeneous-palette argument does not apply.
            constrained = any(value in held for value in (*neighbours, here, there))
            if len(
                [
                    o
                    for o in neighbours
                    if may.get(o, everything) & allowed
                    and len(near.get(o, ())) >= (k if constrained else len(may.get(o, everything)))
                ]
            ) >= k and (
                here in held
                or there in held
                or not (
                    _george(here, there, allowed, near, may, held) or _george(there, here, allowed, near, may, held)
                )
            ):
                continue  # Briggs and George: the merged class would not be colourable
            # Allocation receives pins keyed by the original value ids.
            # Keep the pinned member as the class representative.
            if mine_pin is not None and theirs_pin is None:
                here, there = there, here
            parent[here] = there
            live[there] = merged
            live.pop(here, None)
            may[there] = allowed
            may.pop(here, None)
            widths[there] = width
            widths.pop(here, None)
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
        inputs=frozenset(swap.get(value, value) for value in body.inputs),
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


def _george(gone: int, kept: int, allowed: frozenset, near: dict, may: dict, held: dict) -> bool:
    """Whether `gone` can join `kept` without making `kept` harder to colour.

    George's test, for the join Briggs refuses because the class is long
    and busy: a loop counter and its increment, whose copy is one
    instruction long, share every neighbour but the few live across that
    instruction. Each neighbour of `gone` either already constrains `kept`,
    cannot take a register the class may, or has fewer neighbours than
    registers of its own -- and `kept` keeps its whole palette. A pinned
    neighbour cannot be coloured last, so only the first two answer for it.
    """
    everything = frozenset(target.AVAILABLE)
    if allowed != may.get(kept, everything):
        return False
    return all(
        other in near.get(kept, ())
        or not (may.get(other, everything) & allowed)
        or (other not in held and len(near.get(other, ())) < len(may.get(other, everything)))
        for other in near.get(gone, set()) - {gone, kept}
    )


def _interference(body: lir.LirBody) -> dict[int, set[int]]:
    from qbopt.backend import allocate

    incoming, outgoing = allocate.live(body)
    widths: dict[int, int] = {}
    for one in body.insns:
        operands = (*one.what.dests, *one.what.sources) if one.what is not None else ()
        held = [value for operand in operands for value in ir.values(operand)]
        held.extend(value for value, _ in (*one.requires, *one.delivers))
        for value in held:
            widths[value.value] = max(widths.get(value.value, 0), value.width)
        for value, width in one.widths:
            widths[value] = max(widths.get(value, 0), width)
    graph: dict[int, set[int]] = {}

    def edge(one: int, other: int) -> None:
        if one != other:
            graph.setdefault(one, set()).add(other)
            graph.setdefault(other, set()).add(one)

    targets = {to for block in body.blocks for to in block.succ}
    entries = {body.entry} | {block.at for block in body.blocks if block.at not in targets}
    for block in body.blocks:
        if block.at in entries:
            for value in incoming[block.at]:
                for other in incoming[block.at]:
                    edge(value, other)
        alive = set(outgoing[block.at])
        index = len(block.insns) - 1
        while index >= 0:
            one = block.insns[index]
            if one.group is not None:
                first = index
                while first > 0 and block.insns[first - 1].group == one.group:
                    first -= 1
                group = block.insns[first : index + 1]
                # The values live after a parallel copy all coexist.  This
                # is normally recorded one instruction at a time below, but
                # a group deliberately has no instruction order: treating
                # its destinations as though one were written before the
                # next let two loop phis with the same initial value become
                # one value.  crosscall's running total and loop counter
                # both started at zero, then the generated loop compared the
                # total against its bound instead of the counter.
                for value in alive:
                    for other in alive:
                        edge(value, other)
                # A phi-copy group happens simultaneously: every source is
                # live before any destination is written.  Reading the
                # printed order as execution order let fibonacci64 merge
                # `next` with the still-needed `current`; its check returned
                # 1 instead of 0.
                before = (alive - {value for item in group for value in item.defines}) | {
                    value for item in group for value in item.uses
                }
                for value in before:
                    for other in before:
                        edge(value, other)
                for item in group:
                    for value in item.defines:
                        graph.setdefault(value, set())
                alive = before
                index = first - 1
                continue
            copy = _copy(one)
            equal = None
            if copy is not None and one.defines == (copy[0],) and one.uses == (copy[1],):
                into, source = one.what.dests[0], one.what.sources[0]
                if into.width == source.width == widths.get(copy[0]) == widths.get(copy[1]):
                    equal = copy[1]
            for value in one.defines:
                for other in alive:
                    if other != equal:
                        edge(value, other)
            alive.difference_update(one.defines)
            alive.update(one.uses)
            index -= 1
        for value in block.arrives:
            for other in alive:
                edge(value, other)
    return graph


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


def _wants(side: tuple, swap: dict[int, int]) -> tuple:
    """A requirement, naming the value that survived the join.

    A register an instruction demands is a fact about the instruction, and
    joining two values does not lift it. Left naming the value that was
    joined away, the restore idiom's requirement pointed at a value
    nothing defined any more: the spiller gave it a frame slot, reloaded
    from it, and nothing had ever stored to it.
    """
    return tuple((ir.Held(swap.get(held.value, held.value), held.width), r) for held, r in side)


def _renamed(one: lir.Insn, swap: dict[int, int]) -> lir.Insn:
    """One instruction with every joined value naming its survivor."""
    if one.what is None:
        return replace(
            one,
            defines=tuple(swap.get(v, v) for v in one.defines),
            uses=tuple(swap.get(v, v) for v in one.uses),
            requires=_wants(one.requires, swap),
            delivers=_wants(one.delivers, swap),
            widths=tuple((swap.get(v, v), w) for v, w in one.widths),
        )
    what = one.what
    return replace(
        one,
        what=replace(
            what,
            dests=tuple(_settled(x, swap) for x in what.dests),
            sources=tuple(_settled(x, swap) for x in what.sources),
        ),
        defines=tuple(swap.get(v, v) for v in one.defines),
        uses=tuple(swap.get(v, v) for v in one.uses),
        requires=_wants(one.requires, swap),
        delivers=_wants(one.delivers, swap),
        widths=tuple((swap.get(v, v), w) for v, w in one.widths),
    )


def _settled(where: ir.Loc | ir.Held, swap: dict[int, int]) -> ir.Loc | ir.Held:
    return ir.mapped(where, lambda value: ir.Held(swap.get(value.value, value.value), value.width))
