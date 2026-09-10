"""Make values leaving a natural loop explicit at its exit.

Loop transforms need one place to find everything a loop produces.  LCSSA
provides it: a value defined in the loop and read after it is carried through
an exit phi, so later passes need not search through arbitrary downstream
uses.

The current raised corpus already has a dedicated exit and one exiting edge
for every natural loop.  Those are the only loops changed here.  Multiple
exits need CFG canonicalisation first; guessing a merge for them would make a
foundational invariant unsound.
"""

from dataclasses import replace

from qbopt.analysis import loops, ssa
from qbopt.model import mir
from qbopt.model.passes import MIRTransform


class LoopClosedSSA(MIRTransform):
    name = "lcssa"

    def transform(self, body: mir.MirBody) -> mir.MirBody:
        return closed(body)


def closed(body: mir.MirBody) -> mir.MirBody:
    """Return ``body`` with every supported natural loop in closed SSA form."""
    result = body
    # loops() deliberately returns inner loops first.  Closing an inner loop
    # first makes its exit value an ordinary definition in an enclosing loop.
    for loop in loops.loops(result.blocks, result.entry):
        result = _closed_loop(result, loop)
    return result


def _closed_loop(body: mir.MirBody, loop: loops.Loop) -> mir.MirBody:
    blocks = {block.at: block for block in body.blocks}
    exiting = [
        (block.at, successor)
        for block in body.blocks
        if block.at in loop.body
        for successor in block.succ
        if successor in blocks and successor not in loop.body
    ]
    if len(exiting) != 1:
        return body

    source, exit_at = exiting[0]
    predecessors = loops.predecessors(body.blocks)
    if predecessors[exit_at] != frozenset({source}):
        return body

    defined = {
        value
        for block in body.blocks
        if block.at in loop.body
        for value in (
            *(phi.result for phi in block.phis),
            *(value for op in block.ops for value in op.defines),
        )
        if not value.flags
    }
    if not defined:
        return body

    # Phi inputs are used on their incoming edge.  A phi in the dedicated
    # exit is already the LCSSA boundary, so only downstream phis count here.
    use_sites: dict[mir.Value, set[int]] = {}
    for block in body.blocks:
        if block.at in loop.body:
            continue
        for op in block.ops:
            for value in op.uses:
                if value in defined:
                    use_sites.setdefault(value, set()).add(block.at)
        if block.at == exit_at:
            continue
        for phi in block.phis:
            for predecessor, value in phi.incoming.items():
                if value in defined:
                    use_sites.setdefault(value, set()).add(predecessor)

    dominators = loops.dominators(body.blocks, body.entry)
    crossing = [
        value
        for value, sites in use_sites.items()
        if sites and all(exit_at in dominators.get(site, frozenset()) for site in sites)
    ]
    if not crossing:
        return body

    values = tuple(ssa.values(body))
    next_id = max((value.id for value in values), default=-1) + 1
    next_version = {
        variable: max(one.version for one in values if one.variable == variable) + 1
        for variable in {one.variable for one in crossing}
    }
    swap: dict[int, mir.Value] = {}
    phis: list[mir.Phi] = []
    for offset, value in enumerate(sorted(crossing, key=lambda one: (one.variable, one.version, one.id))):
        # LCSSA closes the live range; it does not invent a new source-level
        # variable.  Keeping the variable identity is what lets recurrence
        # analysis continue through this phi.
        result = mir.Value(next_id + offset, exit_at, variable=value.variable, version=next_version[value.variable])
        next_version[value.variable] += 1
        swap[value.id] = result
        phis.append(mir.Phi(result, {source: value}))

    def rewritten(block: mir.MirBlock) -> mir.MirBlock:
        if block.at in loop.body or exit_at not in dominators.get(block.at, frozenset()):
            return block
        existing = block.phis
        if block.at != exit_at:
            existing = tuple(
                replace(
                    phi,
                    incoming={
                        predecessor: ssa.provider(value, swap)
                        if exit_at in dominators.get(predecessor, frozenset())
                        else value
                        for predecessor, value in phi.incoming.items()
                    },
                )
                for phi in existing
            )
        return replace(
            block,
            phis=existing + (tuple(phis) if block.at == exit_at else ()),
            ops=tuple(ssa.substituted(op, swap) for op in block.ops),
        )

    return replace(body, blocks=tuple(rewritten(block) for block in body.blocks))
