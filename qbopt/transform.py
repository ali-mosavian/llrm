"""
MIR transforms: a body in, an optimised body out.

Everything else in this project changes a program by patching BC's bytes.
This changes the values, and layout.py turns what is left into bytes -- so a
transform here says what the program computes and nothing about how it was
written. That is the difference the roadmap calls retiring the machine arm.

Three of them so far, each the MIR statement of a pass that already exists
against the machine code, and each measured there first:

  widening      wide.pairs() finds the add/adc pair; lift.py emits it today
  redundant     avail.redundant() finds the reload; forward.py deletes it
  dead stores   avail.dead_stores() finds it; memory.py deletes it

Two of the three are on and find nothing extra, which is what parity means:
rewrite.py has already run forward.py and memory.py by the time a body
reaches here, so there is nothing left for them to take. Their worth is that
the machine arm can be deleted, not that they save a byte today. Widening is
off and applied() says why.

**Every byte of the original body has to stay accounted for.** layout.py
refuses a body it cannot cover, which is how it catches data BC put between
the instructions, and a transform that deletes an op leaves a hole in that
arithmetic. So nothing here deletes bytes: a survivor takes over the range,
through `Op.covers`, and the ops that were there are gone from the list.
That is bookkeeping about the *input*, not about what gets emitted -- the
output is whatever the surviving ops select to, which is shorter.

Each transform is separately switchable, deliberately. They interact -- a
widened pair changes which loads are redundant -- and a wrong answer from
one is otherwise a bisect through all three.
"""

from dataclasses import replace

from qbopt import ir
from qbopt import mir
from qbopt import wide
from qbopt import avail
from qbopt import layout
from qbopt.mir import Op
from qbopt.mir import MirBody


def _end_of(op: Op) -> int:
    """One past this op's last original byte."""
    if op.covers is not None:
        return op.covers[1]
    if op.node is None:
        return op.at
    return ir.span(op.node)[1]


def _absorb(ops: list[Op], gone: set[int]) -> list[Op]:
    """`ops` without the ones in `gone`, their bytes given to a survivor.

    Backwards, so a run of deletions collapses onto the one op before them
    rather than each taking the next. The first op in a block has nothing
    before it, so a deletion there is refused by giving it to the op after
    -- and where there is neither, the body is one op long and there is
    nothing to delete.
    """
    if not gone:
        return ops
    out: list[Op] = []
    for op in ops:
        if op.at in gone:
            # The bytes go to the op immediately before, and only if that op
            # is adjacent and gets its length from select.py. Anything
            # further back would span the survivors in between and count
            # their bytes twice; anything emitted verbatim is exactly as long
            # as the bytes it copies, so giving it more to account for makes
            # it disagree with itself -- qb-qrender's SCREEN.OBJ, whose
            # restore idiom stopped coming back its own length.
            #
            # Where neither holds the op simply stays. A deletion this cannot
            # account for is not one worth making.
            if out and layout.selectable(out[-1]) and _end_of(out[-1]) == op.at:
                lo = out[-1].covers[0] if out[-1].covers is not None else out[-1].at
                out[-1] = replace(out[-1], covers=(lo, _end_of(op)))
                continue
            out.append(op)
            continue
        out.append(op)
    return out


def widened(body: MirBody) -> MirBody:
    """Every add/adc pair as the one 32-bit operation it computes.

    Only adjacent halves. The pair's low half keeps its node, so the fixup
    behind its memory operand is still found and still moves with it, and
    takes over both instructions' bytes. A pair with anything between its
    halves is left alone: the widened op would have to claim the bytes in
    between, and whatever is in them is not part of this operation.
    """
    # Which block each op is in, so a pair straddling two can be refused.
    # Adjacent addresses in different blocks means the high half is a
    # branch target: something jumps to it, and folding it into the
    # instruction before would leave that jump landing inside one.
    home = {op.at: block.at for block in body.blocks for op in block.ops}

    found: dict[int, tuple[ir.Semantics, int]] = {}
    gone: set[int] = set()
    for pair in wide.pairs(body):
        if _end_of(pair.low) != pair.high.at:
            continue
        if home.get(pair.low.at) != home.get(pair.high.at):
            continue
        made = wide.widened(pair)
        if made is None:
            continue
        found[pair.low.at] = (made, _end_of(pair.high))
        gone.add(pair.high.at)

    blocks = []
    for block in body.blocks:
        ops = []
        for op in block.ops:
            got = found.get(op.at)
            if got is not None:
                made, end = got
                lo = op.covers[0] if op.covers is not None else op.at
                ops.append(replace(op, made=made, covers=(lo, end)))
                continue
            if op.at in gone:
                continue
            ops.append(op)
        blocks.append(replace(block, ops=tuple(ops)))
    return replace(body, blocks=tuple(blocks))


def without_redundant_loads(body: MirBody, dgroup: frozenset[int], calls: dict[int, str]) -> MirBody:
    """Every load whose destination already held what it loads, removed."""
    gone = set(avail.redundant(body, dgroup, calls))
    if not gone:
        return body
    return replace(
        body,
        blocks=tuple(replace(one, ops=tuple(_absorb(list(one.ops), gone))) for one in body.blocks),
    )


def without_dead_stores(body: MirBody, dgroup: frozenset[int], calls: dict[int, str]) -> MirBody:
    """Every store overwritten before anything read it, removed."""
    gone = set(avail.dead_stores(body, dgroup, calls))
    if not gone:
        return body
    return replace(
        body,
        blocks=tuple(replace(one, ops=tuple(_absorb(list(one.ops), gone))) for one in body.blocks),
    )


def applied(
    body: MirBody,
    dgroup: frozenset[int],
    calls: dict[int, str],
    *,
    widen: bool = False,
    drop_loads: bool = True,
    drop_stores: bool = True,
    place: bool = False,
) -> MirBody:
    """Every transform this module has, in the order they help each other.

    Widening first, because folding a pair retires the carry between its
    halves and removes an op -- which is what makes a later reload of the
    same cell visible as redundant rather than as the high half's own read.

    **Widening is off, and wrong as written.** `add ax,[x]` with
    `adc dx,[x+2]` is a 32-bit add of a value BC keeps in `dx:ax`, and
    `dx:ax` is not `eax` -- so folding it to `add eax,[x]` puts the carry
    into the high half of eax and leaves dx holding what it held before.
    suite/procs.bas prints 0x02040C10 where it wants 0x04080C10: the low
    half doubled, the high half untouched. Every one of the twelve
    configurations caught it.

    Making it right means what lift.py already does -- proving the pair is
    one value and putting it in one register first -- which is the pair
    analysis, not a rename of the low half's operands. wide.widened() builds
    the operation; what is missing is the step before it.
    """
    if widen:
        body = widened(body)
    if drop_loads:
        body = without_redundant_loads(body, dgroup, calls)
    if drop_stores:
        body = without_dead_stores(body, dgroup, calls)
    # Off by default. Sinking a definition is the only transform here that
    # changes the order instructions run in, and its own benefit is
    # indirect -- shorter live ranges, which other passes then use. It gets
    # switched on when something asks for that, not before.
    if place:
        body = placed(body)
    return body


def _may_move(op: Op) -> bool:
    """Whether moving this op within its block could change the program.

    Refused outright: a barrier, whose behaviour is its encoding's; a call,
    which is a barrier for everything this does not model; anything that
    touches memory, because two accesses only commute when they provably do
    not alias and this asks a cheaper question than that; and anything
    defining or using the flags, because a comparison and the branch reading
    it are joined by a value whose live range is one instruction and which
    nothing may be placed inside.
    """
    if op.barrier or op.loads or op.stores:
        return False
    if any(one.flags for one in (*op.defines, *op.uses)):
        return False
    return op.node is not None or op.made is not None


def _placed(ops: list[Op], origin: dict) -> list[Op]:
    """`ops` with each movable definition as late as its uses allow.

    Sinking, not hoisting: a definition moved down to just before the first
    op that reads it shortens its live range, which is the whole point --
    the value stops occupying a register across everything in between.

    One pass, backwards, and only within the block. A definition with no use
    in this block cannot move, because its use is somewhere this cannot see
    and "as late as its uses allow" has no answer.
    """
    first_use: dict[int, int] = {}
    for index, op in enumerate(ops):
        for value in op.uses:
            first_use.setdefault(value.id, index)

    out = list(ops)
    for index in range(len(out) - 1, -1, -1):
        op = out[index]
        if not _may_move(op):
            continue
        made = [one for one in op.defines if not one.flags]
        if len(made) != 1:
            continue
        wanted = first_use.get(made[0].id)
        if wanted is None or wanted <= index + 1:
            continue
        # Two things stop it, and the second is the one SSA hides. Nothing
        # between here and there may write what this op reads, or the value
        # it computes is a different one. And nothing between may touch the
        # *register* this op writes -- values are per-definition and
        # registers are shared, so sinking a definition of ax past another
        # definition of ax leaves this one clobbering it, and past a read of
        # ax leaves that read seeing the wrong value.
        reads = {one.id for one in op.uses}
        into = origin.get(made[0])
        blocked = False
        for other in out[index + 1 : wanted]:
            if other.barrier or any(value.id in reads for value in other.defines):
                blocked = True
                break
            if into is not None and any(
                origin.get(value) is into for value in (*other.defines, *other.uses)
            ):
                blocked = True
                break
        if blocked:
            continue
        moved = out.pop(index)
        out.insert(wanted - 1, moved)
    return out


def placed(body: MirBody) -> MirBody:
    """Every movable definition sunk to just before its first use.

    M2, and the reason the roadmap puts it before LICM: nothing can be
    hoisted out of a loop while an op's position is its address. Here a
    block's op list is the order they are emitted in -- layout._ordered
    stopped sorting by address for exactly this -- so a transform may
    reorder within a block and the bytes follow.

    What it buys directly is shorter live ranges, which is what
    `simplify._target_is_free` and `avail.py` refuse sites over today.
    """
    return replace(
        body,
        blocks=tuple(replace(one, ops=tuple(_placed(list(one.ops), body.origin))) for one in body.blocks),
    )


# What each absorbed runtime routine computes, and the machine operation it
# becomes. Named here rather than imported from calls.py, which is the arm
# M5 retires; the contracts themselves are runtime.py's, read out of the
# QuickBASIC 4.5 source.
ABSORB = {
    "B$MUI4": ("imul", ir.Operation.MULTIPLY),
    "B$DVI4": ("idiv", ir.Operation.DIVIDE),
    "B$RMI4": ("idiv", ir.Operation.DIVIDE),
    "B$CPI4": ("cmp", ir.Operation.COMPARE),
}

# Which argument is pushed first. B$CPI4 takes its left operand first and the
# three arithmetic routines take it last -- the one asymmetry between them,
# and getting it backwards is a different answer, not a slower one.
LEFT_FIRST = {"B$CPI4": True, "B$MUI4": False, "B$DVI4": False, "B$RMI4": False}


def _slot(ref: mir.MemRef) -> int | None:
    """The stack depth this reference names, or None if it is not one."""
    from qbopt.module import Space

    return ref.addr.disp if ref.addr is not None and ref.addr.space is Space.STACK else None


def arguments(body: MirBody, calls: dict[int, str]) -> dict[int, tuple[mir.Value, ...]]:
    """Each absorbable call's arguments, as the values that were pushed.

    Block-scoped, and the whole point of mir._stack_slot keeping the depth
    across a recognised call: an argument pushed before some *other* call
    runs is stranded under it, and 831 of the corpus's 923 absorbable calls
    have something other than a push immediately before them.

    A push whose slot this cannot name gives up on that call rather than
    guessing, and so does a call whose arguments are not all still on the
    stack where they were put.
    """
    found: dict[int, tuple[mir.Value, ...]] = {}
    for block in body.blocks:
        live: dict[int, mir.Value] = {}
        for op in block.ops:
            name = calls.get(op.at)
            if name is not None and name.upper() in ABSORB:
                wanted = 2
                # Top of stack first, so the deepest slot is the argument
                # pushed earliest.
                deep = sorted(live)
                if len(deep) >= wanted:
                    taken = tuple(live[one] for one in deep[:wanted])
                    if name.upper() not in LEFT_FIRST or not LEFT_FIRST[name.upper()]:
                        taken = tuple(reversed(taken))
                    found[op.at] = taken
                live = {}
                continue
            if op.barrier:
                live = {}
                continue
            for ref in op.stores:
                where = _slot(ref)
                if where is None:
                    continue
                reading = [one for one in op.uses if not one.flags]
                if len(reading) == 1:
                    live[where] = reading[0]
    return found
