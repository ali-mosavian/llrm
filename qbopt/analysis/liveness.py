"""Which values are live where, over MIR's own SSA values.

Nothing here names a register. It sat in regalloc.py because that is what
first needed it, and a MIR pass asking regalloc a liveness question is a
MIR pass calling a machine one -- rule 5's own example. The allocator is
one caller of this; the hoist and the fold are others.
"""

from dataclasses import dataclass

from qbopt.model import mir
from qbopt.model.mir import Value


@dataclass(frozen=True, slots=True)
class Liveness:
    live_in: dict[int, frozenset[Value]]
    live_out: dict[int, frozenset[Value]]


def _defines(block: mir.MirBlock) -> set[Value]:
    return {phi.result for phi in block.phis} | {one for op in block.ops for one in op.defines}


def _exposed(block: mir.MirBlock) -> set[Value]:
    """Values the ordinary operations read before writing.

    A phi's arguments belong to the edges they arrive on, so they are the
    predecessor's business and never this block's. Phi results remain here
    when an operation reads them: that demand decides whether the matching
    edge argument is live at all.
    """
    live: set[Value] = set()
    for op in reversed(block.ops):
        live -= set(op.defines)
        live |= set(op.uses)
    return live


def entry_values(body: mir.MirBody) -> frozenset[Value]:
    """Values the caller supplied: used somewhere, defined by nothing here.

    A body reads a register before writing it whenever BC passes something
    in, and mir._Namer invents a value for it rather than pretending the
    read has no operand. Nothing in the body defines one, so unless the
    entry block is told it does, backward liveness never kills it: it stays
    live at every point that can reach its use, which around a loop is
    everywhere. That reported twelve values live at once in a body BC
    compiled into six registers -- impossible, and the reason pressure()
    treats a number above len(AVAILABLE) as a bug rather than a spill.
    """
    defined = {one for block in body.blocks for one in _defines(block)}
    used = {
        one
        for block in body.blocks
        for one in (
            {u for op in block.ops for u in (*op.uses, *op.exits)}
            | {v for phi in block.phis for v in phi.incoming.values()}
        )
    }
    return frozenset(used - defined)


def pressure(
    body: mir.MirBody, found: "Liveness | None" = None, inside: "frozenset[int] | set[int] | None" = None
) -> int:
    """The most values live at once, flags aside -- anywhere, or in `inside`.

    Cannot exceed the number of registers BC itself used, because the
    program came out of them, so a number above the register file is a bug
    in the liveness and not a body that needs spilling.

    `inside` asks it of one loop's blocks, which is what a pass deciding
    whether it can afford another loop-carried value has to know: a body's
    peak says nothing about the loop the value would live around.
    """
    found = found or live(body)
    peak = 0
    for block in body.blocks:
        if inside is not None and block.at not in inside:
            continue
        alive = set(found.live_out[block.at])
        peak = max(peak, len([one for one in alive if not one.flags]))
        for op in reversed(block.ops):
            alive -= set(op.defines)
            alive |= set(op.uses)
            peak = max(peak, len([one for one in alive if not one.flags]))
    return peak


def phi_inputs(body: mir.MirBody, found: "Liveness | None" = None) -> frozenset[Value]:
    """The edge operands of phis whose results are actually live.

    SSA construction may create phis for machine state which no operation
    observes.  Such a phi is not a use of every incoming value.  Walk from
    ordinary live-out back to the point immediately after the phis, then
    select only the incoming edges of demanded results.
    """
    found = found or live(body)
    inputs: set[Value] = set()
    for block in body.blocks:
        alive = set(found.live_out[block.at])
        for op in reversed(block.ops):
            alive.difference_update(op.defines)
            alive.update(op.uses)
        inputs.update(incoming for phi in block.phis if phi.result in alive for incoming in phi.incoming.values())
    return frozenset(inputs)


def live(body: mir.MirBody) -> Liveness:
    """What is live at each block's entry and exit, to a fixed point."""
    # Phi definitions happen before ordinary operations and read one selected
    # predecessor edge. Keeping the two kinds of definition separate avoids
    # making every syntactic phi input live when the phi result is dead.
    op_defines = {block.at: {one for op in block.ops for one in op.defines} for block in body.blocks}
    phi_defines = {block.at: {phi.result for phi in block.phis} for block in body.blocks}
    exposed = {block.at: _exposed(block) for block in body.blocks}
    if body.blocks:
        # Defined at the top of the entry block, before its first
        # instruction: added to what it defines AND removed from what it
        # leaves upward-exposed. Only the first is not enough -- live_in is
        # (live_out - defines) + exposed, so an exposed use puts the value
        # straight back and the kill never happens.
        arriving = set(entry_values(body))
        op_defines[body.entry] |= arriving
        exposed[body.entry] -= arriving
    live_in: dict[int, set[Value]] = {block.at: set() for block in body.blocks}
    live_out: dict[int, set[Value]] = {block.at: set() for block in body.blocks}
    after_phis: dict[int, set[Value]] = {block.at: set() for block in body.blocks}

    changing = True
    while changing:
        changing = False
        for block in body.blocks:
            # An exit observation happens after the operation carrying it.
            # Seeding live-out, rather than treating it as an operand, lets a
            # terminal call define the value its caller observes without also
            # pretending that the call reads its own result.
            out: set[Value] = {value for op in block.ops for value in mir.exit_values(op)}
            for successor in block.succ:
                found = body.block(successor)
                if found is None:
                    continue
                out |= live_in[successor]
                out |= {
                    phi.incoming[block.at]
                    for phi in found.phis
                    if phi.result in after_phis[successor] and block.at in phi.incoming
                }
            after = (out - op_defines[block.at]) | exposed[block.at]
            inside = after - phi_defines[block.at]
            if out != live_out[block.at] or inside != live_in[block.at] or after != after_phis[block.at]:
                live_out[block.at], live_in[block.at], after_phis[block.at] = out, inside, after
                changing = True

    return Liveness(
        {at: frozenset(what) for at, what in live_in.items()},
        {at: frozenset(what) for at, what in live_out.items()},
    )
