"""Registers for a lowered body: the cheapest assignment, not the first one.

Two things separate this from `regalloc.colour()`, which it will replace.

**Spilling is priced, not counted.** `intervals.weights()` has the formula
and it is LLVM's: references weighted by loop depth, divided by how long
the value is live. The division is the half a plain count misses -- two
values referenced equally often are not equally worth keeping if one is
live for three instructions and the other for the whole body.

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

from qbopt import intervals
from qbopt import ir
from qbopt import target
from qbopt import lir
from qbopt.passes import LIRTransform

# How many assignments the search will consider before it gives up and says
# so. Bodies in this corpus colour in a few hundred; the cap is for the one
# that would not.
BUDGET = 200_000


class Unplaced(Exception):
    """A value reached emission with no register. Always a bug here."""


class Spilled(Exception):
    """A value the allocator chose to spill, and nothing writes the spill.

    Choosing is half of it. LLVM's InlineSpiller then rewrites every
    definition of the value into a store and every use into a load, and
    PrologEpilogInserter grows the frame to hold the slot. Neither phase
    exists here, so a body that needs one is refused by name rather than
    emitted with an operand pointing nowhere.
    """


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

    `intervals.py` answers the same question precisely enough to split a
    range; this answers it precisely enough to colour, and is what the
    search below walks. The two agree on overlap.
    """
    _into, out_of = live(body)
    graph: dict[int, set[int]] = {}

    def meet(alive: set[int]) -> None:
        for one in alive:
            graph.setdefault(one, set()).update(other for other in alive if other != one)

    for block in body.blocks:
        # Every value the block names, so that one defined and immediately
        # dead still gets a register. Liveness never sees it -- it is in no
        # live set anywhere -- and the operand naming it is still emitted.
        for one in block.insns:
            for value in (*one.defines, *one.uses):
                graph.setdefault(value, set())
        for value in block.arrives:
            graph.setdefault(value, set())
        alive = set(out_of[block.at])
        meet(alive)
        for one in reversed(block.insns):
            alive -= set(one.defines)
            alive |= set(one.uses)
            meet(alive)
    return {one: frozenset(others) for one, others in graph.items()}


def allocate(body: lir.LirBody, pinned: dict[int, Register_] | None = None) -> Assignment:
    """The cheapest register assignment this body admits."""
    graph = interference(body)
    price = intervals.weights(body)
    fixed = dict(pinned or {})
    order = sorted(graph, key=lambda one: (-price.get(one, 0.0), -len(graph[one]), one))
    confined = classes(body)
    free = {one: list(target.order(confined.get(one))) for one in graph}

    greedy = _greedy(order, graph, fixed, free, price)
    best, whole = _searched(order, graph, fixed, free, price, greedy)
    # A search that finished proves its answer, and that includes finishing
    # without beating the bound -- then the greedy assignment *is* the
    # cheapest, and saying otherwise would understate what is known.
    return Assignment(best.where, best.spilled, best.cost, whole, "" if whole else "the search ran out")


def classes(body: lir.LirBody) -> dict[int, frozenset]:
    """The register class each value is confined to, where it is confined.

    LLVM allocates within a `TargetRegisterClass` and orders the candidates
    with an `AllocationOrder`; asking "any of the six" is only right when
    every operand can take any of the six. 16-bit addressing reaches memory
    through bx, bp, si and di and nothing else -- `[dx+0Ah]` has no
    encoding -- so a value some instruction reaches a cell by is confined
    to that class, and an allocator that does not know it will eventually
    hand out dx.

    Only the addressing class today. The fixed requirements -- `imul`'s
    dx:ax, `cwd`'s eax, a shift's cl -- arrive as pins from the raise and
    are already honoured; `target.reads()` and `target.writes()` are what
    would answer them here when they do not.
    """
    out: dict[int, frozenset] = {}
    for block in body.blocks:
        for one in block.insns:
            if one.what is None:
                continue
            for where in (*one.what.dests, *one.what.sources):
                if isinstance(where, ir.Mem) and isinstance(where.through, ir.Held):
                    out[where.through.value] = target.ADDRESSING
    return out


def _allowed(one: int, fixed: dict, free: list) -> list:
    """The registers this value may take, in the order to try them."""
    want = fixed.get(one)
    return [want] if want is not None else list(free.get(one, ()))


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


class RegAlloc(LIRTransform):
    """Assign, then rewrite. LLVM's two halves, in one phase.

    `RegAllocBase` assigns virtual registers to physical ones and
    `VirtRegRewriter` walks the function afterwards replacing each operand.
    They are separate passes there because the assignment is a analysis
    result other passes read -- stack slot colouring, copy propagation --
    and nothing here reads it yet. `allocate()` and `applied()` are the two
    halves and stay separable.
    """

    name = "regalloc"

    # How many times a body may be spilled and re-allocated. Each round
    # frees the registers the round before could not, and the corpus
    # settles in one; the cap is for a body where a reload's own value
    # cannot be placed either, which would otherwise loop.
    ROUNDS = 4

    def __init__(self, pinned: dict | None = None, frame=None) -> None:
        self.pinned = pinned or {}
        self.frame = frame

    def transform(self, body: lir.LirBody) -> lir.LirBody:
        """Assign; where that spills, make the spill real and assign again.

        LLVM's `RegAllocBase::allocatePhysRegs` is this loop: a value it
        cannot place is spilled, the spiller puts the new short intervals
        back, and the queue is worked again. Splitting comes first there --
        `splitkit.py` here -- and spilling is what is left when no split
        helps.
        """
        from qbopt import frame as frames
        from qbopt import spiller

        if self.frame is None:
            self.frame = frames.of(body)
        for _round in range(self.ROUNDS):
            got = allocate(body, self.pinned)
            if not got.spilled:
                return applied(body, got)
            body = spiller.spilled(body, got.spilled, self.frame)
        got = allocate(body, self.pinned)
        return applied(body, got)


def applied(body: lir.LirBody, got: Assignment) -> lir.LirBody:
    """`body` with every operand naming a value replaced by its register.

    A value the allocation has no answer for keeps whatever the raise saw
    it in. That is the honest fallback: the operand the original
    instruction had in that position is what a pass took away.
    """
    if got.spilled:
        worst = sorted(got.spilled)[:4]
        raise Spilled(
            f"{len(got.spilled)} values want a stack slot ({', '.join(f'value#{one}' for one in worst)}) "
            f"at a cost of {got.cost:g}; the spiller and the frame that holds the slots are not written"
        )
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
    # The register file's table. Asking the encoder which register is which
    # would be the allocator reaching down a tier for a fact about the
    # machine, which target.py exists to hold.
    return ir.Reg(target.named(register, where.width), where.width)
