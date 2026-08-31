"""
Which blocks are a loop, and how deeply nested each one is.

blocks.py answers "where does control go"; this answers "what does control
come back to". Nothing else in this pass has needed that -- widening and call
absorption are both scoped to a single block, where a loop is invisible --
but every optimisation that decides what is worth doing needs it, for two
different reasons.

The first is priority. A redundant load in a loop that runs 25,000 times is
worth 25,000 times what the same load costs in straight-line setup code, and
nothing in the object says which is which except the loop structure. That is
not a hypothetical: this pass spent real effort on a multiply in
bench/nbody.bas's own PITSNAP, which reads the PC timer exactly twice per
run, because a disassembly makes cold code and hot code look identical.

depth() is only half of that answer, and the same example is why. PITSNAP
busy-waits on the timer, so its own blocks sit two loops deep and score
*higher* than most of the program while running twice in total. Nesting is
within one body; how often that body is entered is a question about the call
graph, which nothing here builds. So this ranks blocks against their own
neighbours honestly and across procedures not at all -- enough to stop
mistaking setup code for a kernel, not enough to rank two kernels.

One shape defeats it outright, and it is worth knowing which. RESUME compiles
to a dispatch block that branches back to every point the handler can return
to; each of those dominates it, so each is a natural loop by the definition
above, and the block comes back nested 28 to 33 deep. That is what the
definition says rather than a defect in it, but it is not nesting in any
sense an optimiser should weight. Measured across the 110 fixtures: every
object except divmod -- the one program built with /X, the switch that lets a
handler RESUME -- has a maximum depth of 1. Resumable error handling is out
of the optimiser's scope, so this is a reason to refuse such a body rather
than a number to fix.

The second is legality. Hoisting anything out of a loop needs to know what
the loop *is* -- its header, and every block that can reach the latch without
leaving through the header.

Standard machinery, deliberately: iterative dominators to a fixed point, then
a back edge is an edge to a block that dominates its own source, and a
natural loop is that edge's header plus everything that reaches the latch
without passing through the header again. What is not standard is what
happens when the answer is not a natural loop at all. BASIC has GOTO, so a
BC-compiled module can be irreducible -- a cycle entered at two different
blocks, where no single one of them dominates the rest. `irreducible()`
reports those rather than forcing them into a shape they do not have, and a
caller that cannot reason about one is expected to refuse it, the way every
other refusal in this pass works.
"""

from dataclasses import dataclass

from qbopt.blocks import Block

# The entry every module's own control flow starts from -- blocks.ENTRY, but
# taken from the block list rather than assumed, since a body that is not the
# main one starts wherever its own PUBDEF put it.


def predecessors(blocks: list[Block]) -> dict[int, frozenset[int]]:
    """Who can reach each block, inverted from its own successors."""
    known = {block.at for block in blocks}
    found: dict[int, set[int]] = {block.at: set() for block in blocks}
    for block in blocks:
        for successor in block.succ:
            if successor in known:
                found[successor].add(block.at)
    return {at: frozenset(who) for at, who in found.items()}


def dominators(blocks: list[Block], entry: int | None = None) -> dict[int, frozenset[int]]:
    """Every block that must have run before each one, to a fixed point.

    A block unreachable from the entry gets the empty set rather than "every
    block", which is what the usual initialise-to-everything formulation
    would leave it with. Nothing here is reachable-only by construction --
    blocks.py seeds from real entry points, but a body's own blocks are
    handed here as a list, and an unreachable one would otherwise come back
    claiming to be dominated by blocks that never run.
    """
    if not blocks:
        return {}
    start = entry if entry is not None else blocks[0].at
    every = frozenset(block.at for block in blocks)
    preds = predecessors(blocks)

    doms = {block.at: every for block in blocks}
    doms[start] = frozenset({start})

    changing = True
    while changing:
        changing = False
        for block in blocks:
            if block.at == start:
                continue
            reaching = [doms[one] for one in preds[block.at] if one in doms]
            now = (frozenset.intersection(*reaching) if reaching else frozenset()) | {block.at}
            if now != doms[block.at]:
                doms[block.at] = now
                changing = True
    # a block no path reaches dominates nothing, itself included
    for block in blocks:
        if block.at != start and not preds[block.at]:
            doms[block.at] = frozenset()
    return doms


@dataclass(frozen=True, slots=True)
class Loop:
    """One natural loop: where control comes back to, and what is inside.

    Keyed by header, not by back edge. A loop can be re-entered from more
    than one place -- BC emits that for a FOR whose body carries its own
    EXIT FOR, and for any loop a GOTO jumps back into -- and those are one
    loop with several latches, not several loops. Counting them separately
    makes a singly-nested block look nested once per latch, which on this
    corpus reached a nesting depth of 33.
    """

    header: int
    latches: frozenset[int]  # every block whose own edge goes back to the header
    body: frozenset[int]  # every block in the loop, the header included


def back_edges(blocks: list[Block], doms: dict[int, frozenset[int]]) -> list[tuple[int, int]]:
    """(latch, header) for every edge to a block that dominates its source."""
    known = {block.at for block in blocks}
    return [
        (block.at, successor)
        for block in blocks
        for successor in block.succ
        if successor in known and successor in doms.get(block.at, frozenset())
    ]


def _body(latch: int, header: int, preds: dict[int, frozenset[int]]) -> frozenset[int]:
    """Everything that reaches the latch without going back through the header.

    The header goes in before the walk starts, which is what stops it: every
    path out of the loop leaves through the header, so refusing to expand
    through it bounds the walk to the loop itself. A block whose own edge
    goes back to itself is the case that breaks if the walk starts anyway --
    expanding the header's predecessors would pull in everything that
    reaches the loop rather than everything inside it.
    """
    body = {header}
    if latch == header:
        return frozenset(body)
    body.add(latch)
    pending = [latch]
    while pending:
        at = pending.pop()
        for one in preds.get(at, frozenset()):
            if one not in body:
                body.add(one)
                pending.append(one)
    return frozenset(body)


def loops(blocks: list[Block], entry: int | None = None) -> list[Loop]:
    """Every natural loop, innermost first where they nest.

    Back edges sharing a header are one loop whose body is the union of
    theirs -- see Loop's own note on why they are not several.
    """
    doms = dominators(blocks, entry)
    preds = predecessors(blocks)

    latches: dict[int, set[int]] = {}
    bodies: dict[int, set[int]] = {}
    for latch, header in back_edges(blocks, doms):
        latches.setdefault(header, set()).add(latch)
        bodies.setdefault(header, set()).update(_body(latch, header, preds))

    found = [Loop(header, frozenset(latches[header]), frozenset(bodies[header])) for header in latches]
    return sorted(found, key=lambda loop: len(loop.body))


def irreducible(blocks: list[Block], entry: int | None = None) -> frozenset[int]:
    """Blocks left in a cycle once every natural loop's back edge is cut.

    A control-flow graph is reducible exactly when deleting its back edges --
    the ones whose target dominates their source, which loops() already
    finds -- leaves no cycle at all. Whatever cycle survives that is entered
    at more than one block, so it has no header to hoist to or out of, and a
    caller that cannot reason about one should refuse it.

    Decided by actually cutting the edges and looking for a remaining cycle,
    not by address order. A retreating address is not the same question: BC
    compiles a FOR with its test at the bottom (`jmp test`, then the body,
    then the test branching back up into it), so the edge into the body runs
    backwards through the address space while the loop is perfectly
    reducible with the test as its header. An address-order rule calls every
    one of those irreducible, which on bench/nbody.bas alone is six spurious
    refusals.

    Reported rather than repaired -- node splitting would change what the
    object's own control flow is, and this pass does not do that anywhere.
    """
    doms = dominators(blocks, entry)
    known = {block.at for block in blocks}
    cut = set(back_edges(blocks, doms))
    forward = {block.at: [s for s in block.succ if s in known and (block.at, s) not in cut] for block in blocks}

    # three-colour DFS: grey is the current stack, so an edge into it closes
    # a cycle that survived the cut
    WHITE, GREY, BLACK = 0, 1, 2
    colour = dict.fromkeys(forward, WHITE)
    found: set[int] = set()

    for start in forward:
        if colour[start] != WHITE:
            continue
        stack = [(start, iter(forward[start]))]
        colour[start] = GREY
        while stack:
            at, pending = stack[-1]
            for successor in pending:
                if colour[successor] == GREY:
                    found.add(successor)
                elif colour[successor] == WHITE:
                    colour[successor] = GREY
                    stack.append((successor, iter(forward[successor])))
                    break
            else:
                colour[at] = BLACK
                stack.pop()
    return frozenset(found)


def depth(blocks: list[Block], entry: int | None = None) -> dict[int, int]:
    """How many loops each block is inside -- 0 for straight-line code.

    A count rather than a trip estimate because the object does not carry
    one: a FOR with constant bounds could be read back out of the code, but
    a WHILE could not, and being honest that one loop beats none is more
    useful than guessing by how much.

    Comparable only within a body -- see the module docstring on PITSNAP.
    """
    found = {block.at: 0 for block in blocks}
    for loop in loops(blocks, entry):
        for at in loop.body:
            if at in found:
                found[at] += 1
    return found


def immediate_dominators(blocks: list[Block], entry: int | None = None) -> dict[int, int | None]:
    """Each block's nearest strict dominator, or None for the entry and for
    anything unreachable.

    Read off the full dominator sets rather than computed by the usual
    Lengauer-Tarjan walk: dominators() already runs to a fixed point over
    graphs of a few dozen blocks, and the nearest of a block's strict
    dominators is simply the one with the most dominators of its own -- it
    is furthest from the entry, and dominance along a path is a total order.
    """
    doms = dominators(blocks, entry)
    found: dict[int, int | None] = {}
    for block in blocks:
        strict = doms.get(block.at, frozenset()) - {block.at}
        found[block.at] = max(strict, key=lambda one: len(doms[one])) if strict else None
    return found


def frontiers(blocks: list[Block], entry: int | None = None) -> dict[int, frozenset[int]]:
    """Where a definition stops being the only one that reaches -- the blocks
    a phi belongs in.

    A block is on n's frontier when n dominates one of its predecessors but
    not the block itself: control arrives there both through n and around it,
    so two definitions meet. Cytron's own walk, which only ever climbs from a
    join's predecessors to its immediate dominator, so a block with one
    predecessor can never be on anyone's frontier.
    """
    idom = immediate_dominators(blocks, entry)
    preds = predecessors(blocks)
    found: dict[int, set[int]] = {block.at: set() for block in blocks}
    for block in blocks:
        if len(preds[block.at]) < 2:
            continue
        for one in preds[block.at]:
            runner = one
            while runner is not None and runner != idom[block.at]:
                found[runner].add(block.at)
                runner = idom.get(runner)
    return {at: frozenset(where) for at, where in found.items()}
