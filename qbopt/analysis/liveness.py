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
    """Values this block reads before writing -- phi arguments excluded.

    A phi's arguments belong to the edges they arrive on, so they are the
    predecessor's business and never this block's.
    """
    live: set[Value] = set()
    for op in reversed(block.ops):
        live -= set(op.defines)
        live |= set(op.uses)
    return live - {phi.result for phi in block.phis}


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
        for one in ({u for op in block.ops for u in op.uses} | {v for phi in block.phis for v in phi.incoming.values()})
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


def live(body: mir.MirBody) -> Liveness:
    """What is live at each block's entry and exit, to a fixed point."""
    defines = {block.at: _defines(block) for block in body.blocks}
    exposed = {block.at: _exposed(block) for block in body.blocks}
    if body.blocks:
        # Defined at the top of the entry block, before its first
        # instruction: added to what it defines AND removed from what it
        # leaves upward-exposed. Only the first is not enough -- live_in is
        # (live_out - defines) + exposed, so an exposed use puts the value
        # straight back and the kill never happens.
        arriving = set(entry_values(body))
        defines[body.entry] |= arriving
        exposed[body.entry] -= arriving
    live_in: dict[int, set[Value]] = {block.at: set() for block in body.blocks}
    live_out: dict[int, set[Value]] = {block.at: set() for block in body.blocks}

    changing = True
    while changing:
        changing = False
        for block in body.blocks:
            out: set[Value] = set()
            for successor in block.succ:
                found = body.block(successor)
                if found is None:
                    continue
                out |= live_in[successor]
                out |= {phi.incoming[block.at] for phi in found.phis if block.at in phi.incoming}
            inside = (out - defines[block.at]) | exposed[block.at]
            if out != live_out[block.at] or inside != live_in[block.at]:
                live_out[block.at], live_in[block.at] = out, inside
                changing = True

    return Liveness(
        {at: frozenset(what) for at, what in live_in.items()},
        {at: frozenset(what) for at, what in live_out.items()},
    )
