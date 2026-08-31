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
            if out:
                lo = out[-1].covers[0] if out[-1].covers is not None else out[-1].at
                out[-1] = replace(out[-1], covers=(lo, _end_of(op)))
                continue
            # Nothing before it: hand the bytes forward instead, by leaving
            # the op in place. A deletion this cannot account for is not one
            # worth making.
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
    widen: bool = True,
    drop_loads: bool = True,
    drop_stores: bool = True,
) -> MirBody:
    """Every transform this module has, in the order they help each other.

    Widening first, because folding a pair retires the carry between its
    halves and removes an op -- which is what makes a later reload of the
    same cell visible as redundant rather than as the high half's own read.
    """
    if widen:
        body = widened(body)
    if drop_loads:
        body = without_redundant_loads(body, dgroup, calls)
    if drop_stores:
        body = without_dead_stores(body, dgroup, calls)
    return body
