"""Registers for a lowered body: the cheapest assignment, not the first one.

Two things separate this from `regalloc.colour()`, which it will replace.

**Spilling is priced, not counted.** A value spilled inside a loop pays for
every iteration, and the loop is where all of this project's programs spend
their time -- so the cost of keeping a value in memory is the number of
times it is read or written, each weighted by how deeply nested the block
holding it is. The usual factor of ten per level: one reference in a doubly
nested loop outweighs a hundred outside.

**The assignment is searched, not greedy.** Greedy colouring in the order
values are most constrained is optimal on a chordal graph with nothing
pre-coloured, and neither half holds here: a barrier pins every register it
touches, an absorbed divide pins eax and edx, and BC's own calling
convention pins more. Branch and bound over the values, most expensive
first, pruning as soon as the spill bill reaches the best answer so far.
The search has a node budget; where it runs out this says so and the
greedy answer stands, rather than pretending the result is optimal.
"""

from dataclasses import dataclass
from dataclasses import replace

from iced_x86 import Register_

from qbopt import ir
from qbopt import lir
from qbopt import loops as loopy

# One reference costs this much more per level of loop nesting. The classic
# 10, which is what makes an inner-loop value beat a straight-line one that
# is referenced nine times as often.
PER_LEVEL = 10

# How many assignments the search will consider before it gives up and says
# so. Bodies in this corpus colour in a few hundred; the cap is for the one
# that would not.
BUDGET = 200_000


class Unplaced(Exception):
    """A value reached emission with no register. Always a bug here."""


@dataclass(frozen=True, slots=True)
class Assignment:
    """Where each value lives, what it cost, and whether that is the best.

    Keyed by value id, which is what a lowered operand names. The allocator
    used to be handed the MIR body instead, so that it could read
    `op.defines` for its interference graph -- a LIR pass reaching back up a
    form to ask a question its own input can now answer.
    """

    where: dict[int, Register_]
    spilled: frozenset[int]
    cost: float
    optimal: bool
    why: str = ""


def depths(body: lir.LirBody) -> dict[int, int]:
    """How deeply each block is nested in loops.

    `loops.loops()` gives every natural loop's body, so a block's depth is
    how many of them contain it. Keyed by header, so a loop with several
    latches counts once -- counting back edges instead reached a nesting
    depth of 33 on this corpus.
    """
    found = loopy.loops(list(body.blocks), body.entry)
    out = {block.at: 0 for block in body.blocks}
    for loop in found:
        for at in loop.body:
            if at in out:
                out[at] += 1
    return out


def costs(body: lir.LirBody) -> dict[int, float]:
    """What keeping each value in memory would cost, by weighted references.

    A definition and a use both count: spilled, the first becomes a store
    and the second a load, and BC's own code is already full of both --
    which is the whole reason this project exists.
    """
    deep = depths(body)
    out: dict[int, float] = {}
    for block in body.blocks:
        weight = float(PER_LEVEL ** deep.get(block.at, 0))
        for one in block.insns:
            for value in (*one.defines, *one.uses):
                out[value] = out.get(value, 0.0) + weight
    return out


def live(body: lir.LirBody) -> tuple[dict[int, set[int]], dict[int, set[int]]]:
    """What is live at each block's entry and exit, to a fixed point.

    LIR's own, over the ids an operand names. liveness.py answers the same
    question over MIR values and is what the passes above use; the two are
    the same algorithm on the two forms, and neither can read the other's.
    """
    defines = {block.at: set(block.arrives) | {v for one in block.insns for v in one.defines} for block in body.blocks}
    exposed = {}
    for block in body.blocks:
        alive: set[int] = set()
        for one in reversed(block.insns):
            alive -= set(one.defines)
            alive |= set(one.uses)
        exposed[block.at] = alive - set(block.arrives)

    live_in = {block.at: set() for block in body.blocks}
    live_out = {block.at: set() for block in body.blocks}
    changing = True
    while changing:
        changing = False
        for block in body.blocks:
            out = set()
            for at in block.succ:
                out |= live_in.get(at, set())
            into = exposed[block.at] | (out - defines[block.at])
            if out != live_out[block.at] or into != live_in[block.at]:
                live_out[block.at], live_in[block.at] = out, into
                changing = True
    return live_in, live_out


def interference(body: lir.LirBody) -> dict[int, frozenset[int]]:
    """Which values are ever live at the same moment.

    Walked backwards over each block's own live set rather than by
    comparing ranges: two values interfere exactly when both are in that
    set at some point, which is the definition and needs no approximation.
    """
    _into, out_of = live(body)
    graph: dict[int, set[int]] = {}

    def meet(alive: set[int]) -> None:
        for one in alive:
            graph.setdefault(one, set()).update(other for other in alive if other != one)

    for block in body.blocks:
        alive = set(out_of[block.at])
        meet(alive)
        for one in reversed(block.insns):
            alive -= set(one.defines)
            alive |= set(one.uses)
            meet(alive)
        for value in block.arrives:
            graph.setdefault(value, set())
    return {one: frozenset(others) for one, others in graph.items()}


def allocate(body: lir.LirBody, pinned: dict[int, Register_] | None = None) -> Assignment:
    """The cheapest register assignment this body admits."""
    from qbopt import regalloc

    graph = interference(body)
    price = costs(body)
    fixed = dict(pinned or {})
    order = sorted(graph, key=lambda one: (-price.get(one, 0.0), -len(graph[one]), one))
    free = list(regalloc.AVAILABLE)

    greedy = _greedy(order, graph, fixed, free, price)
    best, whole = _searched(order, graph, fixed, free, price, greedy)
    # A search that finished proves its answer, and that includes finishing
    # without beating the bound -- then the greedy assignment *is* the
    # cheapest, and saying otherwise would understate what is known.
    return Assignment(best.where, best.spilled, best.cost, whole, "" if whole else "the search ran out")


def _allowed(one: int, fixed: dict, free: list) -> list:
    """The registers this value may take."""
    want = fixed.get(one)
    return [want] if want is not None else list(free)


def _clashes(one: int, register, graph: dict, where: dict) -> bool:
    return any(where.get(other) == register for other in graph.get(one, ()))


def _greedy(order, graph, fixed, free, price) -> Assignment:
    """The first assignment that works, for a bound the search can beat."""
    where: dict[int, Register_] = {}
    spilled: set[int] = set()
    cost = 0.0
    for one in order:
        got = next((r for r in _allowed(one, fixed, free) if not _clashes(one, r, graph, where)), None)
        if got is None:
            spilled.add(one)
            cost += price.get(one, 0.0)
            continue
        where[one] = got
    return Assignment(where, frozenset(spilled), cost, False, "greedy")


def _searched(order, graph, fixed, free, price, bound: Assignment) -> tuple[Assignment, bool]:
    """Branch and bound over the same order, keeping the cheapest.

    Most expensive value first, so the branch that spills it is cut almost
    at once -- which is what makes the search finish at all.
    """
    best = bound
    seen = 0

    def walk(index: int, where: dict, spilled: frozenset, cost: float):
        nonlocal best, seen
        seen += 1
        if seen >= BUDGET or cost >= best.cost:
            return
        if index == len(order):
            best = Assignment(dict(where), spilled, cost, True, "")
            return
        one = order[index]
        for register in _allowed(one, fixed, free):
            if _clashes(one, register, graph, where):
                continue
            where[one] = register
            walk(index + 1, where, spilled, cost)
            del where[one]
            if seen >= BUDGET:
                return
        walk(index + 1, where, spilled | {one}, cost + price.get(one, 0.0))

    walk(0, {}, frozenset(), 0.0)
    return best, seen < BUDGET


def applied(body: lir.LirBody, got: Assignment) -> lir.LirBody:
    """`body` with every operand naming a value replaced by its register.

    A value the allocation has no answer for keeps whatever the raise saw
    it in. That is the honest fallback: the operand the original
    instruction had in that position is what a pass took away.
    """
    held = got.where
    return lir.LirBody(
        name=body.name,
        entry=body.entry,
        blocks=tuple(
            lir.LirBlock(
                at=block.at,
                insns=tuple(_placed(one, held, body.origin) for one in block.insns),
            )
            for block in body.blocks
        ),
        origin=body.origin,
        pins=body.pins,
    )


def _placed(one: lir.Insn, held: dict, origin: dict) -> lir.Insn:
    if one.what is None:
        return one
    what = one.what
    dests = tuple(_settled(x, held, origin) for x in what.dests)
    sources = tuple(_settled(x, held, origin) for x in what.sources)
    if dests == what.dests and sources == what.sources:
        return one
    return replace(one, what=ir.Semantics(what.op, what.name, dests, sources, what.target))


def _settled(where, held: dict, origin: dict):
    """One operand with its value resolved to the register holding it."""
    if not isinstance(where, ir.Held):
        return where
    register = held.get(where.value)
    if register is None:
        # No fallback. An operand the allocation does not cover is the
        # allocator having missed a value, and quietly putting back what
        # the raise saw hides that -- the caller then gets a body that
        # emits, one instruction of which is wrong for a reason nothing
        # reported. Named here, where it is still known which value.
        raise Unplaced(f"value#{where.value} at width {where.width} has no register")
    from qbopt import regalloc

    # regalloc's table, not select's. Both hold the same map and asking the
    # encoder which register is which is the allocator reaching down a tier
    # for a fact about the register file.
    return ir.Reg(regalloc._named(register, where.width), where.width)
