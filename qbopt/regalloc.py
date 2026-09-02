"""
Which register each value lives in.

BC allocates nothing: it keeps values in memory and reaches for a register
only long enough to compute with one. Everything measured about this pass's
remaining opportunity comes back to that -- of 384 forwardable loads, 308
need the value to end up in a different register from the one BC wrote it
to, which is not a rewrite anything could emit until something decides where
values live.

SSA is what makes the deciding tractable. Interference in an SSA program is
a chordal graph, so a greedy colouring in dominance order is optimal -- no
iterated coalescing, no build-simplify-select loop. The whole allocator is
liveness, then one walk down the dominator tree.

Liveness here is the standard formulation and the phi handling is the part
worth stating, because it is where a plausible-looking version goes wrong:

    live_out(B) = union over successors S of
                      live_in(S)  +  {phi.incoming[B] for phi in phis(S)}
    live_in(B)  = (live_out(B) - defs(B)) + upward-exposed uses(B)

A phi argument is live out of the *predecessor it comes from*, not live in
to the block holding the phi -- it is a value on an edge. Attributing it to
the phi's own block instead makes every argument of every phi live at the
join simultaneously, which reports pressure the program never had. A first
attempt at this measured a peak of twelve live values in a body BC compiled
into six registers, which is impossible: renaming does not change what is
live, only what it is called.
"""

from dataclasses import dataclass

from iced_x86 import Register
from iced_x86 import Register_

from qbopt import ir
from qbopt import mir
from qbopt.mir import NAMES
from qbopt.mir import Value

# What there is to allocate into. The flags are not a register anything can
# be put in, and mir.FLAGS values are excluded from pressure and colouring
# alike -- wide.py's own measurement is that they all fold away or become a
# comparison, so they are not competing for anything.
AVAILABLE: tuple[Register_, ...] = mir.TRACKED

# 16-bit addressing reaches memory through bx, bp, si or di and nothing
# else: `[dx+0Ah]` has no encoding. bp is the frame pointer and is not on
# offer, so a value some instruction reaches a cell by can go in three
# places. Without this the allocator put segld's array base in dx, the
# selector refused the operand, and the body came back with the definition
# renamed and every use of it left behind.
ADDRESSING: frozenset[Register_] = frozenset({Register.BX, Register.SI, Register.DI})


def _addressing(body: mir.MirBody) -> set[Value]:
    """Every value some instruction reaches a memory operand by."""
    found: set[Value] = set()
    for block in body.blocks:
        for op in block.ops:
            what = op.made if op.made is not None else getattr(op.node, "semantics", None)
            if what is None:
                continue
            reached = {
                where
                for one in (*what.dests, *what.sources)
                for where in (
                    getattr(one, "through", None),
                    getattr(one, "index", None),
                    getattr(getattr(one, "addr", None), "base", None),
                )
                if where is not None and where is not Register.NONE
            }
            roots = {ir.ROOT.get(one, one) for one in reached}
            found |= {value for value in op.uses if ir.ROOT.get(body.origin.get(value, -1), -1) in roots}
    return found


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


def pressure(body: mir.MirBody, found: Liveness | None = None) -> int:
    """The most values live at once anywhere in this body, flags aside.

    Cannot exceed the number of registers BC itself used, because the
    program came out of them -- so a number above len(AVAILABLE) is a bug
    in the liveness and not a body that needs spilling.
    """
    found = found or live(body)
    peak = 0
    for block in body.blocks:
        alive = set(found.live_out[block.at])
        peak = max(peak, len([one for one in alive if not one.flags]))
        for op in reversed(block.ops):
            alive -= set(op.defines)
            alive |= set(op.uses)
            peak = max(peak, len([one for one in alive if not one.flags]))
    return peak


def interference(body: mir.MirBody, found: Liveness | None = None) -> dict[Value, frozenset[Value]]:
    """Which values are ever live at the same moment.

    Built by walking each block backwards over its own live set rather than
    by comparing live ranges: two values interfere exactly when both are in
    that set at some point, which is the definition and needs no
    approximation. Flags are excluded -- they are not a register anything
    is placed in.
    """
    found = found or live(body)
    graph: dict[Value, set[Value]] = {}

    def meet(alive: set[Value]) -> None:
        real = [one for one in alive if not one.flags]
        for one in real:
            graph.setdefault(one, set()).update(other for other in real if other != one)

    for block in body.blocks:
        alive = set(found.live_out[block.at])
        meet(alive)
        for op in reversed(block.ops):
            alive -= set(op.defines)
            alive |= set(op.uses)
            meet(alive)
        for phi in block.phis:
            # Not a flags phi. meet() keeps flags out of the graph
            # everywhere else, and this line put them back: segld's latch
            # merges one, so f1 entered the graph here, asked for a
            # register in the greedy pass, found NONE was not one it could
            # use, and took eax from a value that wanted it.
            if not phi.result.flags:
                graph.setdefault(phi.result, set())
    return {one: frozenset(others) for one, others in graph.items()}


def colour(body: mir.MirBody, pinned: dict[Value, Register_] | None = None) -> dict[Value, Register_] | str:
    """A register for every value, or why there is not one.

    Greedy over the interference graph, in order of how constrained each
    value is. In SSA that ordering is optimal without a spill loop, because
    the graph is chordal -- but only while nothing is pre-coloured, and
    plenty here is: a barrier pins every register it touches, a call
    clobbers what runtime.py says it does, and `cwd`, `idiv` and a shift by
    cl each demand particular ones. Pre-colouring breaks the guarantee, so
    this can fail, and failing is a refusal to allocate the body rather
    than a licence to guess.

    The identity assignment -- every value back into the register BC used --
    is always valid, since that is where the program came from. So a
    failure here is never "this body cannot be allocated"; it is "this
    body cannot be allocated *the way something asked for*".
    """
    graph = interference(body)
    assigned: dict[Value, Register_] = dict(pinned or {})
    reached_by = _addressing(body)
    wide_addressing = {ir.ROOT.get(one, one) for one in ADDRESSING}
    for one in reached_by:
        want = assigned.get(one)
        if want is not None and ir.ROOT.get(want, want) not in wide_addressing:
            return f"{one} is how a cell is reached and {NAMES.get(want, want)} cannot reach one"
    # Where BC had each value. Read from the body rather than off the value
    # itself: this is the one question in this module that is genuinely
    # about machine registers, and asking for it explicitly is the point of
    # the map living there. See docs/variables.md.
    origin = body.origin

    # Identity first, and it is not a heuristic: measured over 27,680 values,
    # none ever interferes with another version of its own register, so
    # putting every value back where BC had it is always a valid colouring.
    # An allocator that moves anything it was not asked to move is emitting
    # copies for nothing, and greedy-by-degree moved 1,274 values doing
    # exactly that. Only a pin, or a clash with one, makes anything move.
    if not any(assigned[one] is not origin.get(one) for one in assigned):
        clean = {one: origin[one] for one in graph if one in origin}
        clean.update(assigned)
        if all(
            all(clean[other] is not clean[one] for other in graph[one] if other in clean)
            for one in clean
            if one in graph
        ):
            return clean

    for one, others in sorted(graph.items(), key=lambda kv: (-len(kv[1]), kv[0].id)):
        if one in assigned:
            continue
        taken = {assigned[other] for other in others if other in assigned}
        offer = AVAILABLE
        if one in reached_by:
            offer = [where for where in AVAILABLE if ir.ROOT.get(where, where) in wide_addressing]
        free = [where for where in offer if where not in taken]
        if not free:
            return f"{one} interferes with every register at once"
        # its own register first, so an allocation that need not move
        # anything does not
        was = origin.get(one)
        assigned[one] = was if was is not None and was in free else free[0]

    # A phi is not an instruction. Nothing runs on the edge to move a value
    # into place, so a phi's result and every value arriving at it have to
    # already be in one register -- and this colours them independently.
    # segld's inner counter was defined into dx and read back out of ax
    # through the phi between them, so it never reached its bound and the
    # program ran forever.
    for block in body.blocks:
        for phi in block.phis:
            if phi.result.flags:
                continue
            want = assigned.get(phi.result)
            for value in phi.incoming.values():
                if assigned.get(value) is not want:
                    came, goes = NAMES.get(assigned.get(value)), NAMES.get(want)
                    return f"{phi.result} arrives from {value} and nothing moves it: {came} into {goes}"

    # The pins are not checked on the way in, so they are checked here: two
    # of them wanting one register for values that are live together is a
    # refusal, and without this the loop hands them straight back because a
    # value already assigned is skipped rather than validated.
    for one, others in graph.items():
        for other in others:
            if one.flags or other.flags:
                continue
            if one in assigned and other in assigned and assigned[one] is assigned[other]:
                return f"{one} and {other} are live together and both want {NAMES.get(assigned[one], assigned[one])}"
    return assigned


def moved(body: mir.MirBody, assignment: dict[Value, Register_]) -> int:
    """How many values ended up somewhere other than BC put them."""
    return sum(1 for one, where in assignment.items() if where is not body.origin.get(one))
