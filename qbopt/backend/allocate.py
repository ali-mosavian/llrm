"""Registers for a lowered body, using greedy allocation with eviction.

Two things separate this from `regalloc.colour()`, which it will replace.

**Spilling is priced, not counted.** `intervals.weights()` has the formula
and it is LLVM's: references weighted by loop depth, divided by how long
the value is live. The division is the half a plain count misses -- two
values referenced equally often are not equally worth keeping if one is
live for three instructions and the other for the whole body.

**Fixed intervals go first.** Hardware requirements reserve their registers
before flexible intervals compete for the rest. Assignment, eviction and
spill stages advance monotonically; the result does not claim optimality.
"""

import heapq
from enum import IntEnum
from collections import Counter
from dataclasses import replace
from typing import TYPE_CHECKING
from dataclasses import dataclass

from iced_x86 import Register
from iced_x86 import Register_

from qbopt.model import ir
from qbopt.model import lir
from qbopt.backend import target
from qbopt.backend import cpu as targets
from qbopt.model.passes import LIRTransform
from qbopt.analysis import intervals as ranges

if TYPE_CHECKING:
    from qbopt.backend.frame import Frame

# Maximum queue visits before the remaining values are spilled.
BUDGET = 200_000

# How long a range may be and still count as a reload. One instruction is
# two slots; a little slack for a value the spiller wrote and something
# immediately after read.
RELOAD = 4 * ranges.PER_INSN


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
        index = len(block.insns) - 1
        while index >= 0:
            one = block.insns[index]
            first = index
            if one.group is not None:
                while first > 0 and block.insns[first - 1].group == one.group:
                    first -= 1
            group = block.insns[first : index + 1]
            alive -= {value for item in group for value in item.defines}
            alive |= {value for item in group for value in item.uses}
            index = first - 1
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


def narrowed(body: lir.LirBody, pinned: "dict[int, Register_]") -> "tuple[lir.LirBody, dict[int, Register_]]":
    """A call's results that nothing reads, said as clobbers instead.

    The raise gives a call a result per register it cannot prove the
    routine preserves, and pins each to the register it comes back in.
    Where nothing reads one, that pin reserves a register for a value that
    does not exist -- and the spiller cannot help, since storing a dead
    value frees nothing: bools-q-evt spilled 118, then 119, then 120,
    one per round, and never settled.

    The register is still destroyed, so it moves to `clobbers`, which is
    what keeps a live range out of it. Only a call, only a result nothing
    reads, and only after the phases that rewrite reads have run.
    """
    read = {value for block in body.blocks for one in block.insns for value in one.uses}
    read |= {value for block in body.blocks for value in block.arrives}
    dropped: dict[int, Register_] = {}
    blocks = []
    for block in body.blocks:
        insns = []
        for one in block.insns:
            dead = [value for value in one.defines if value not in read and pinned.get(value) is not None]
            if one.what is None or one.what.op is not ir.Operation.CALL or not dead:
                insns.append(one)
                continue
            gone: dict[int, Register_] = {value: pinned[value] for value in dead}
            dropped.update(gone)
            insns.append(
                replace(
                    one,
                    defines=tuple(value for value in one.defines if value not in gone),
                    delivers=tuple((held, register) for held, register in one.delivers if held.value not in gone),
                    clobbers=one.clobbers | frozenset(gone.values()),
                )
            )
        blocks.append(replace(block, insns=tuple(insns)))
    if not dropped:
        return body, pinned
    return replace(body, blocks=tuple(blocks)), {
        value: where for value, where in pinned.items() if value not in dropped
    }


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
        index = len(block.insns) - 1
        while index >= 0:
            one = block.insns[index]
            first = index
            if one.group is not None:
                while first > 0 and block.insns[first - 1].group == one.group:
                    first -= 1
            group = block.insns[first : index + 1]
            alive -= {value for item in group for value in item.defines}
            alive |= {value for item in group for value in item.uses}
            meet(alive)
            index = first - 1
    return {one: frozenset(others) for one, others in graph.items()}


class Stage(IntEnum):
    """How far a range has got, and therefore what may still be tried on it.

    LLVM's `LiveRangeStage`. A range that fails to assign is not simply
    spilled: it is put back on the queue one stage further along, so the
    next attempt tries something the last one did not. The stage is what
    stops the same remedy being tried forever.
    """

    ASSIGN = 0  # never tried: a free register, or evict something cheaper
    SPLIT = 1  # eviction did not help: cut the range instead
    SPILL = 2  # nothing helped
    DONE = 3


def classes(body: lir.LirBody) -> dict[int, frozenset[Register_]]:
    """The register class each value is confined to, where it is confined.

    LLVM allocates within a `TargetRegisterClass` and orders the candidates
    with an `AllocationOrder`; asking "any of the six" is only right when
    every operand can take any of the six. 16-bit addressing reaches memory
    through bx, bp, si and di and nothing else -- `[dx+0Ah]` has no
    encoding -- so a value some instruction reaches a cell by is confined
    to that class, and an allocator that does not know it hands out dx.

    Byte operands additionally require one of AX/BX/CX/DX's byte aliases.
    Intersect requirements when a value occurs in more than one class.
    The fixed requirements -- `imul`'s
    dx:ax, `cwd`'s eax, a shift's cl -- arrive as pins from the raise;
    `target.reads()` and `target.writes()` are what would answer them here
    when they do not.
    """
    out: dict[int, frozenset[Register_]] = {}
    # A selector goes in a segment register only where every other operand
    # it appears in can name one: a move, a push or a pop.
    selecting: set[int] = set()
    numeric: set[int] = set()

    def restrict(value: int, choices: frozenset[Register_]) -> None:
        out[value] = out.get(value, choices) & choices

    for block in body.blocks:
        for one in block.insns:
            if one.what is None:
                continue
            for where in (*one.what.dests, *one.what.sources):
                if isinstance(where, ir.Mem) and where.selector is not None:
                    selecting.add(where.selector.value)
                if isinstance(where, ir.Mem) and where.base is not None:
                    numeric.add(where.base.value)
                if isinstance(where, ir.Held) and (where.width != 2 or one.what.op not in _SEGMENT_OPERANDS):
                    numeric.add(where.value)
                # `base` and not `through`: the first is the value that
                # computed the address, the second only how to encode the
                # operand once something has placed it. 16-bit addressing
                # reaches memory through bx, bp, si and di and nothing
                # else, so the value is confined to those.
                # Unless it is scaled: `[base+index*scale]` is 32-bit addressing,
                # which any general register reaches memory through.
                if isinstance(where, ir.Mem) and where.base is not None and where.index is None:
                    restrict(where.base.value, target.ADDRESSING)
                if isinstance(where, ir.Mem) and where.index is not None:
                    numeric.add(where.index.value)
                    if where.index.width == 2:
                        restrict(where.index.value, target.WORD_INDEXES)
                        if where.base is not None:
                            restrict(where.base.value, target.WORD_BASES)
                if isinstance(where, ir.Held) and where.width == 1:
                    restrict(where.value, frozenset({Register.AX, Register.BX, Register.CX, Register.DX}))
    for value in selecting - numeric:
        restrict(value, frozenset(target.SELECTORS))
    return out


_SEGMENT_OPERANDS = frozenset({ir.Operation.MOVE, ir.Operation.PUSH, ir.Operation.POP})


def _unread_move(one: lir.Insn) -> bool:
    """A plain move into one value; whether anything reads it is the caller's."""
    what = one.what
    return (
        what is not None
        and what.op is ir.Operation.MOVE
        and len(what.dests) == 1
        and len(what.sources) == 1
        and isinstance(what.dests[0], ir.Held)
        and isinstance(what.sources[0], (ir.Imm, ir.Held))
        and one.defines == (what.dests[0].value,)
        and not (one.requires or one.delivers or one.clobbers)
        and one.group is None
        and one.symbol is not True
    )


def explicit_selectors(body: lir.LirBody, pinned: "dict[int, Register_] | None" = None) -> lir.LirBody:
    """Each far cell whose selector is also read as a number, or pinned to a
    general register, reached through ES.

    That value cannot live in a segment register, so the cell names ES and the
    instruction requires the value there, the way a call requires an argument:
    `constrain` gives the occurrence a copy of its own.
    """
    confined = classes(body)
    selectors = frozenset(target.SELECTORS)
    conflicted = {
        where.selector.value
        for one in body.insns
        if one.what is not None
        for where in (*one.what.dests, *one.what.sources)
        if isinstance(where, ir.Mem)
        and where.selector is not None
        and (
            confined.get(where.selector.value) != selectors
            or (pinned or {}).get(where.selector.value, Register.ES) not in target.SEGMENTS
        )
    }
    if not conflicted:
        return body

    def through_es(where):
        if isinstance(where, ir.Mem) and where.selector is not None and where.selector.value in conflicted:
            return replace(where, selector=None, addr=replace(where.addr, segment=Register.ES))
        return where

    blocks = []
    for block in body.blocks:
        insns = []
        for one in block.insns:
            named = (
                []
                if one.what is None
                else [
                    where.selector
                    for where in (*one.what.dests, *one.what.sources)
                    if isinstance(where, ir.Mem) and where.selector is not None and where.selector.value in conflicted
                ]
            )
            if not named:
                insns.append(one)
                continue
            what = replace(
                one.what, dests=tuple(map(through_es, one.what.dests)), sources=tuple(map(through_es, one.what.sources))
            )
            requires = tuple(dict.fromkeys((*one.requires, *((held, Register.ES) for held in named))))
            uses = tuple(dict.fromkeys((*one.uses, *(held.value for held in named))))
            insns.append(replace(one, what=what, requires=requires, uses=uses))
        blocks.append(replace(block, insns=tuple(insns)))
    return replace(body, blocks=tuple(blocks))


def _copy_hints(body: lir.LirBody) -> dict[int, list[int]]:
    hints: dict[int, list[int]] = {}
    for one in body.insns:
        match one.what:
            case ir.Semantics(ir.Operation.MOVE, "mov", (ir.Held() as dest,), (ir.Held() as source,)):
                if dest.width == source.width and dest.value != source.value:
                    hints.setdefault(dest.value, []).append(source.value)
                    hints.setdefault(source.value, []).append(dest.value)
    return hints


def allocate(
    body: lir.LirBody,
    pinned: dict[int, Register_] | None = None,
    unspillable: "frozenset[int] | None" = None,
    protected: "frozenset[int] | None" = None,
    preferred: "dict[int, Register_] | None" = None,
    *,
    cpu: "str | targets.Profile" = "386",
) -> Assignment:
    """A register for every value, by LLVM's `RegAllocGreedy`.

    Largest range first, out of a priority queue. For each:

        assign   a register nothing live at the same time is using
        evict    take one from ranges that cost less than this one, and
                 put them back on the queue to find another
        split    give up on one register for the whole range
        spill    give up on a register

    A range that fails one stage comes back at the next, which is what
    makes the loop terminate: `Stage` only ever moves forward.

    **Eviction is what the branch-and-bound search here before was for.**
    That search was exactly optimal and exponential, and it went: this is
    LLVM's answer to the same question and a better one for the reason LLVM
    reached it. The cost model is what decides, and a cheap range moving
    aside for an expensive one is the whole of the decision -- a search
    that finds the same answer by trying everything has only proved the
    cost model right at a price that grows with the body.
    """
    profile = targets.profile(cpu)
    index = ranges.indexed(body)
    live = _fold_priced(body, _sibling_priced(body, ranges.intervals(body, index)), profile)
    masks = _masks(body, index)
    widths = _widest(body)
    # A reload's value is live across one instruction and must have a
    # register: spilling it again puts a load in front of a load and
    # nothing settles. LLVM's `markNotSpillable`, as a weight nothing can
    # outbid rather than as a flag every comparison has to remember.
    #
    # Only while it is still short. LLVM asks `LI.isZeroLength()` for the
    # same reason: a value that reached emission with a long range is not a
    # reload any more, whatever made it, and refusing to spill one that
    # crosses fourteen calls clobbering every register is refusing to
    # compile the program -- bools-q-evt, `value#223 cannot be spilled and
    # no register is free for it`.
    for one in unspillable or ():
        if one in live and live[one].size <= RELOAD:
            live[one] = replace(live[one], weight=float("inf"))
    # A pressure plan may retain a long-lived value because spilling it
    # would recreate a loop-invariant address every iteration.  Unlike a
    # reload, its length is not evidence it may be spilled: the candidate
    # evaluator established a legal alternative for the values it displaces.
    # Keep this separate from `unspillable`, whose short-range restriction is
    # what prevents an accidental long reload from making a body impossible.
    for one in protected or ():
        if one in live:
            live[one] = replace(live[one], weight=float("inf"))
    # Values no slot can hold, which is a different fact from a reload
    # being too short to spill again: spiller.py owns which spills are
    # possible, and choosing one of these as a victim buys nothing --
    # the reload lands tied at the same instruction and the round repeats.
    confined = classes(body)
    fixed = dict(pinned or {})
    hints = _copy_hints(body)

    # What is assigned to each register, as intervals. LLVM's
    # LiveIntervalUnion: the question an allocator asks a thousand times is
    # "does this range overlap anything already in that register", and a
    # per-register list answers it without rebuilding a graph.
    union: dict[Register_, list[int]] = {}
    where: dict[int, Register_] = {}
    stage: dict[int, Stage] = {}
    spilled: set[int] = set()
    cost = 0.0
    # LLVM's eviction cascades. An evicted range goes back to ASSIGN and may
    # evict again, but only ranges of an older cascade or of none: without
    # that, the counter PLASMA evicted in favour of inner-loop temporaries
    # never evicted the cheaper accumulator that had taken a register since,
    # and it went to memory. The cascade order is what terminates.
    cascades: dict[int, int] = {}
    newest = 1

    def queued(value: int) -> tuple[bool, float, int]:
        # Fixed intervals have no alternative placement. Reserve them before
        # flexible ranges, so a late hardware requirement does not evict an
        # otherwise placeable loop value straight into the spill stage.
        return value not in fixed, -_priority(live.get(value), stage.get(value, Stage.ASSIGN)), value

    queue = [queued(one) for one in _values(body)]
    heapq.heapify(queue)
    seen = 0
    while queue and seen < BUDGET:
        seen += 1
        _flexible, _prio, value = heapq.heappop(queue)
        if value in where or value in spilled:
            continue
        at = stage.setdefault(value, Stage.ASSIGN)
        mine = live.get(value)
        if mine is None:
            continue
        order = target.order(confined.get(value)) if value not in fixed else (fixed[value],)
        # A preference is an allocation-order vote, unlike `pinned`: it
        # cannot make an otherwise legal plan unplaceable.  Pressure-plan
        # evaluation uses it to compare counter/temporary roles without
        # pretending an ISA or ABI fact requires a particular register.
        if value not in fixed and (choice := (preferred or {}).get(value)) is not None:
            wanted = _whole(choice)
            order = tuple(one for one in order if _whole(one) is wanted) + tuple(
                one for one in order if _whole(one) is not wanted
            )
        # A 16-bit `[base+index]` form has a non-interchangeable base role:
        # only BX can carry it, while SI/DI carry the index.  A protected
        # loop-scoped address owner is deliberately long-lived over the hot
        # region, so assigning it BX first can make every short field base
        # unplaceable even though SI/DI leave the transient role free.  This
        # is an allocation order, not a pin: BX remains available when the
        # other address registers cannot hold the complete plan.
        if value in (protected or ()) and _reserves_word_base(body, value, confined):
            word = frozenset(_whole(one) for one in target.WORD_BASES)
            order = tuple(one for one in order if _whole(one) not in word) + tuple(
                one for one in order if _whole(one) in word
            )
        if value not in fixed:
            votes = Counter(
                _whole(register)
                for other in hints.get(value, ())
                if (register := fixed.get(other, where.get(other))) is not None
            )
            order = tuple(sorted(order, key=lambda register: -votes[_whole(register)]))

        got = _free(mine, order, union, live, masks, widths.get(value, 4))
        if got is not None:
            where[value] = got
            union.setdefault(_whole(got), []).append(value)
            stage[value] = Stage.DONE
            continue

        def movable(other: int, register: Register_) -> bool:
            if other in fixed:
                return False
            elsewhere = tuple(one for one in target.order(confined.get(other)) if _whole(one) != _whole(register))
            return _free(live[other], elsewhere, union, live, masks, widths.get(other, 4)) is not None

        if at is Stage.ASSIGN:
            evicted = _evict(
                mine,
                order,
                union,
                live,
                masks,
                movable,
                frozenset(fixed) | frozenset(protected or ()),
                widths.get(value, 4),
                cascades.get(value, newest),
                cascades,
            )
            if evicted is not None:
                got, victims = evicted
                if value not in cascades:
                    cascades[value] = newest
                    newest += 1
                for one in victims:
                    union[_whole(got)].remove(one)
                    del where[one]
                    cascades[one] = cascades[value]
                    stage[one] = Stage.ASSIGN
                    heapq.heappush(queue, queued(one))
                where[value] = got
                union.setdefault(_whole(got), []).append(value)
                stage[value] = Stage.DONE
                continue
            stage[value] = Stage.SPLIT
            heapq.heappush(queue, queued(value))
            continue

        # Splitting is a rewrite of the body, not a decision about this
        # assignment, so it is the caller's -- RegAlloc.transform runs
        # splitkit and asks again. Here it means "no register".
        #
        # A pin is not a preference.  `constrain` normally mints a short
        # value for a hard occurrence, but callers may also pin a whole
        # interval.  If two such intervals overlap in the same register,
        # sending either one through the spiller loses the requirement: its
        # reload is free to land in a different register.  There is no
        # legal allocation in that case, so report it instead of returning
        # an assignment whose `spilled` set contradicts `where`.
        if value in fixed:
            raise Unplaced(f"value#{value} cannot be placed in fixed {fixed[value]!r}")
        if mine.weight == float("inf"):
            # It cannot be spilled and it cannot be placed. Saying so is
            # the only honest answer: a reload with nowhere to go means the
            # instruction it feeds needs more registers than exist.
            raise Unplaced(f"value#{value} cannot be spilled and no register is free for it")
        spilled.add(value)
        cost += mine.weight
        stage[value] = Stage.DONE

    return Assignment(where, frozenset(spilled), cost, False, "greedy with eviction")


def _values(body: lir.LirBody) -> list[int]:
    """Every value that wants a register, dead definitions included."""
    out: set[int] = set()
    for block in body.blocks:
        out.update(block.arrives)
        for one in block.insns:
            out.update(one.defines)
            out.update(one.uses)
    return sorted(out)


def _priority(one: "ranges.Interval | None", at: Stage) -> float:
    """Where this range sits in the queue. Larger first, as LLVM does.

    "Assigning larger ranges first" is LLVM's own comment on it: a long
    range has the most ways to conflict, so placing it while the register
    file is empty is the placement most likely to succeed. A range that has
    already failed once is boosted so it is retried before the queue moves
    on to ranges it might evict.
    """
    if one is None:
        return 0.0
    return one.size + (1e6 if at is not Stage.ASSIGN else 0.0)


def _reserves_word_base(body: lir.LirBody, value: int, confined: dict[int, frozenset[Register_]]) -> bool:
    """Whether an address-class value should leave a 16-bit word base free.

    The proof is structural and target-wide: a protected address value may
    prefer SI/DI only when this body also has a distinct indexed memory base
    that needs the target's unique word-base class.  It does not inspect a
    procedure name, source location, or an assigned register.
    """
    choices = confined.get(value)
    if not choices or not choices <= target.ADDRESSING:
        return False
    return any(
        isinstance(where, ir.Mem)
        and isinstance(where.base, ir.Held)
        and isinstance(where.index, ir.Held)
        and where.base.value != value
        and where.index.value != value
        and where.scale == 1
        for one in body.insns
        if one.what is not None
        for where in (*one.what.dests, *one.what.sources)
    )


def _widest(body: lir.LirBody) -> dict[int, int]:
    """How wide each value is anywhere it is read or written."""
    out: dict[int, int] = {}
    for one in body.insns:
        held = [
            where
            for operand in ((*one.what.dests, *one.what.sources) if one.what is not None else ())
            for where in ir.values(operand)
        ]
        held += [where for where, _register in (*one.requires, *one.delivers)]
        for where in held:
            out[where.value] = max(out.get(where.value, 0), where.width)
        for value, width in one.widths:
            out[value] = max(out.get(value, 0), width)
    return out


def _masks(
    body: lir.LirBody, index: "ranges.Indexes"
) -> "list[tuple[int, frozenset[Register_], frozenset[Register_]]]":
    """Every point a register is destroyed without being named, and which.

    LLVM's `LiveIntervals::getRegMaskSlots()`. A call is the only one here.
    The second set is the registers it destroys only the upper half of.

    Rooted, because a mask naming eax destroys ax with it.
    """
    out = []
    for block in body.blocks:
        for one in block.insns:
            if one.clobbers or one.clobbers_high:
                out.append(
                    (
                        index.at[id(one)],
                        frozenset(_whole(register) for register in one.clobbers),
                        frozenset(_whole(register) for register in one.clobbers_high),
                    )
                )
    return out


def _whole(register: Register_) -> Register_:
    """The 32-bit register this one is part of.

    Occupancy is a fact about the register itself, and dx is edx's low
    half: counted apart, the allocator put a long in edx while the restore
    idiom beside it delivered its high half in dx, and lngmix printed
    S= 771897293 for 142900. Every question about what is already in a
    register is asked of the whole of it.
    """
    return ir.ROOT.get(register, register)


def _clobbered(one: "ranges.Interval", register: Register_, masks: list, width: int = 4) -> bool:
    """Whether this range is live across a point that destroys the register.

    LLVM's `checkRegMaskInterference`, and it is what lets a call stop
    inventing a value per register it clobbers: the mask says the register
    does not survive, so nothing live across the call may be in it. Without
    this the drop would be a miscompile rather than an optimisation.

    Live *across*, not merely touching: a value the call itself writes
    starts after the clobber, and one that dies at the call ends before it.
    """
    mine = _whole(register)
    for slot, mask, high in masks:
        if mine not in mask and (mine not in high or width <= 2):
            continue
        if any(seg.start < slot and seg.end > slot + ranges.DEF for seg in one.segments):
            return True
    return False


def _free(
    one: "ranges.Interval", order: tuple, union: dict, live: dict, masks: list, width: int = 4
) -> "Register_ | None":
    """A register nothing live at the same time is using, and no call kills."""
    for register in order:
        if _clobbered(one, register, masks, width):
            continue
        if not any(live[other].overlaps(one) for other in union.get(_whole(register), ()) if other in live):
            return register
    return None


def _evict(
    one: ranges.Interval,
    order: tuple[Register_, ...],
    union: dict[Register_, list[int]],
    live: dict[int, ranges.Interval],
    masks: list[tuple[int, frozenset[Register_]]],
    movable=lambda other, register: False,
    protected: frozenset[int] = frozenset(),
    width: int = 4,
    cascade: int | None = None,
    cascades: "dict[int, int] | None" = None,
) -> tuple[Register_, list[int]] | None:
    """The cheapest register to take, and what has to move out of it.

    Only where everything evicted is cheaper than what wants the register,
    which is LLVM's rule and the whole of the cost model: a range is worth
    a register in proportion to how often it is referenced and how briefly
    it is live, and the expensive one wins. A victim another register is
    free for costs nothing, since it moves rather than splits: a reload
    confined to BX stayed unplaced behind a source reload that SI would take.
    A victim must also belong to an older eviction cascade, or to none, so
    two ranges cannot evict each other forever.
    """
    best = None
    for register in order:
        if _clobbered(one, register, masks, width):
            continue
        victims = [other for other in union.get(_whole(register), ()) if other in live and live[other].overlaps(one)]
        if not victims:
            continue
        # A fixed register occurrence may have a low ordinary spill price,
        # but it has no legal spill rewrite: its reload would no longer be
        # tied to the hardware register.  Do not let a hotter flexible range
        # price that fact as a cheap eviction candidate.
        if any(other in protected for other in victims):
            continue
        if cascade is not None and any((cascades or {}).get(other, 0) >= cascade for other in victims):
            continue
        bill = sum(0.0 if movable(other, register) else live[other].weight for other in victims)
        if bill >= one.weight:
            continue
        if best is None or bill < best[0]:
            best = (bill, register, victims)
    return None if best is None else (best[1], best[2])


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
    # frees the registers the round before could not; the cap is for a body
    # where a reload's own value cannot be placed either, which would
    # otherwise loop. Four was enough while the allocator searched
    # exhaustively; greedy-with-eviction spills more values and in smaller
    # groups, so it wants more rounds to settle.
    ROUNDS = 12

    def __init__(
        self,
        pinned: "dict[int, Register_] | None" = None,
        frame: "Frame | None" = None,
        *,
        cpu: "str | targets.Profile" = "386",
    ) -> None:
        self.pinned: dict[int, Register_] = dict(pinned or {})
        self.frame = frame
        self.cpu = targets.profile(cpu)

    def transform(self, body: lir.LirBody) -> lir.LirBody:
        """Assign; where that spills, make the spill real and assign again.

        LLVM's `RegAllocBase::allocatePhysRegs` is this loop: a value it
        cannot place is spilled, the spiller puts the new short intervals
        back, and the queue is worked again. Splitting comes first there --
        `splitkit.py` here -- and spilling is what is left when no split
        helps.
        """
        from qbopt.backend import spiller
        from qbopt.backend import splitkit
        from qbopt.backend import constrain
        from qbopt.backend import frame as frames

        if self.frame is None:
            self.frame = frames.of(body)
        body = explicit_selectors(body, self.pinned)
        # Once, before anything is placed. A required register is a claim
        # about one instruction -- `imul`'s product is dx:ax and it names
        # neither -- so the operand becomes a value of its own, live across
        # that instruction and nothing else. The originals keep whatever
        # the raise saw them in; the fresh ones are held to the hardware,
        # and because they are different values neither can override the
        # other.
        # The splitter's own answer, kept. A requirement an operand can
        # hold is re-derived below from the operand now holding it; one no
        # operand can hold -- a call reads its arguments in registers it
        # names nowhere -- exists only here, and dropping it left B$ENRA's
        # arguments coloured like any other value.
        # Before anything is required of a value: a result nothing reads
        # is not a value at all, and its register is a clobber.
        body, narrower = narrowed(body, self.pinned)
        self.pinned = narrower
        # A fixed-register definition is a constraint on that occurrence,
        # not a lifetime-long register class.  OIMAD's allocator first saw
        # a runtime result in AX and later used the same value as the base
        # of `es:[base]`, whose 16-bit encoding permits only BX/BP/SI/DI.
        # Keeping the MIR pin on the whole SSA value made the final load
        # `es:[ax]`, an encoding that does not exist.  `delivers` retains
        # the call's real AX requirement; release the incompatible whole-
        # range pin so constrain can split that occurrence and leave the
        # value after its copy in the class its uses require.
        confined = classes(body)
        incompatible = {
            value
            for value, register in self.pinned.items()
            if value in confined and _whole(register) not in {_whole(choice) for choice in confined[value]}
        }
        if incompatible:
            delivered = {held.value for one in body.insns for held, _register in one.delivers}
            missing = incompatible - delivered
            if missing:
                value = min(missing)
                raise Unplaced(
                    f"value#{value} is pinned outside its register class and has no defining occurrence to split"
                )
            self.pinned = {value: register for value, register in self.pinned.items() if value not in incompatible}
        # A selector pinned to a segment register is placed like any other
        # value in its class: ES, FS and GS are one choice, and every read
        # through ES that is not a cell's operand already says so itself.
        self.pinned = {
            value: register
            for value, register in self.pinned.items()
            if not (register in target.SEGMENTS and confined.get(value) == frozenset(target.SELECTORS))
        }
        body, fixed = constrain.constrained(body, self.pinned)
        # Disjoint by construction -- the splitter mints ids above every one
        # the body holds -- and said rather than assumed, because a silent
        # overwrite here is a requirement quietly replaced by a preference.
        clash = {one for one in fixed if one in self.pinned and self.pinned[one] != fixed[one]}
        if clash:
            raise Unplaced(f"value#{sorted(clash)[0]} is pinned and required in different registers")
        prefer = {**self.pinned, **fixed}
        # A value constrain minted is live across one instruction and
        # exists only to be in the register that instruction demands, so
        # spilling it answers a different question: the reload that
        # replaced it carried no requirement, and nested and harr ended
        # with their fixed-register input in no register at all. LLVM
        # says the same as `LiveInterval::markNotSpillable`, and the
        # reloads below are already handed back the same way.
        reloads: frozenset[int] = frozenset(fixed)
        # A joint pressure plan may retain a loop-invariant frame-loaded
        # address base and spill the short index it otherwise displaced.
        # The set is chosen only after comparing the baseline allocation to
        # a legal recovery plan below; it is not a preferred physical
        # register or a frontend-specific hint.
        retained: frozenset[int] = frozenset()

        def assigned(current: lir.LirBody) -> Assignment:
            if retained:
                return allocate(current, self.pinned, reloads, protected=retained, cpu=self.cpu)
            return allocate(current, self.pinned, reloads, cpu=self.cpu)

        # Which values have already been cut. Splitting is bounded per
        # value, not merely per round.
        already: set[int] = set()
        for _round in range(self.ROUNDS):
            # A move into a value nothing reads is not a value. nbody's
            # hoisted 512 kept ecx after rematerialization gave every reader
            # its own copy.
            body = spiller._remove_abandoned(body, {id(one) for one in body.insns if _unread_move(one)})
            # Recomputed every attempt, and merged last. The spiller puts a
            # fresh value at an instruction between rounds, and a
            # requirement is about the instruction rather than about the
            # value that happened to be there when the splitter ran.
            self.pinned = {**prefer, **constrain.required(body)}
            got = assigned(body)
            if not retained and got.spilled:
                # A value live from a cold prelude through a later call can
                # still be the base of every memory access in one hot natural
                # loop.  Whole-range protection prices the later call against
                # every trip through that loop.  Carve the loop-only piece
                # first, then evaluate the same complete recovery rule used
                # for a whole invariant: no choice of source procedure or
                # physical register is involved.
                scoped, keep = splitkit.loop_bases(body, got.spilled)
                if keep:
                    # The natural companion to an address owner is a dying
                    # word index in the same loop.  Materialize that fold
                    # before the trial, not afterwards: its SI/DI is exactly
                    # what lets the retained owners leave BX available for
                    # the transient far-field base.  `spilled()` proves the
                    # fold's lifetime and address legality; values outside a
                    # loop that actually uses a fresh owner are not offered.
                    folded = _scoped_foldable_indexes(scoped, keep)
                    prepared, made = (
                        spiller.spilled(scoped, folded, self.frame) if folded else (scoped, frozenset())
                    )
                    try:
                        trial = allocate(prepared, self.pinned, reloads | made, protected=keep, cpu=self.cpu)
                    except Unplaced:
                        trial = None
                    if (
                        trial is not None
                        and keep.isdisjoint(trial.spilled)
                        # The candidate may spill ordinary cold values into
                        # their normal slots; unlike the former whole-range
                        # plan it has already shortened the retained values.
                        # Admit it only when the complete pre-folded trial
                        # lowers weighted dynamic spill traffic, rather than
                        # treating an arbitrary protected assignment as a
                        # gain because its preferred bases happened to fit.
                        and _traffic(prepared, trial.spilled) < _traffic(body, got.spilled)
                    ):
                        body, retained, got, reloads = prepared, keep, trial, reloads | made
                # The global version is useful where a source cell is stable
                # over the whole body.  Keep it as a separate candidate: a
                # loop-scoped split is not a reason to make a once-used owner
                # live through unrelated cold code.
                if retained:
                    pass
                else:
                    keep = _retainable_bases(body, got.spilled)
                    if keep:
                        try:
                            trial = allocate(body, self.pinned, reloads, protected=keep, cpu=self.cpu)
                        except Unplaced:
                            trial = None
                        # A retained invariant is accepted only when every new
                        # spill has a target-legal direct recovery: it is either
                        # rematerialized, or its word index folds into the base
                        # that access kills.  This compares whole allocations,
                        # rather than raising an owner's priority and hoping the
                        # spiller later finds room for whatever it displaced.
                        if (
                            trial is not None
                            and keep.isdisjoint(trial.spilled)
                            and trial.spilled
                            <= spiller.rematerializable(body, trial.spilled)
                            | spiller.foldable_indexes(body, trial.spilled)
                        ):
                            retained, got = keep, trial
            if not got.spilled:
                return applied(body, got)
            # A retained-base plan is admitted only when every displaced
            # value has the direct recovery proven above.  Its complete
            # alternative is therefore the spill rewrite itself: ordinary
            # splitting or spill-web expansion would replace that evaluated
            # plan with a different one and can consume the register the
            # direct fold intentionally freed.
            if retained:
                body, made = spiller.spilled(body, got.spilled, self.frame)
                reloads |= made
                continue
            # Split before spilling, which is the order RegAllocGreedy
            # uses: a range cut at a loop it never touches may fit where
            # the whole of it did not, and a copy is cheaper than a store
            # and a load. Only the values that failed -- splitting every
            # crossing range on principle cost 12,329 bytes over the
            # corpus and freed nothing.
            # One range at a time, as `RegAllocGreedy` splits one and
            # requeues its pieces. Cut together, PLASMA's pieces in its
            # pixel loop spilled anyway and outweighed the one cut that
            # kept the outer counter in a register, and the batch was
            # thrown away with it. Progress is less memory traffic, not
            # fewer values: a cut makes more values whatever it achieves,
            # and traffic -- references weighted by loop depth, not
            # divided by length -- does not fall for a cut that placed
            # nothing, which only adds its copies.
            improved = False
            sizes = ranges.intervals(body)
            for value in sorted(got.spilled, key=lambda one: (-(sizes[one].size if one in sizes else 0), one)):
                if value in already:
                    continue
                cut = splitkit.split(body, frozenset({value}), already, got.where)
                if cut is body:
                    continue
                after = allocate(cut, {**prefer, **constrain.required(cut)}, reloads, cpu=self.cpu)
                if not after.spilled:
                    return applied(cut, after)
                if _traffic(cut, after.spilled) < _traffic(body, got.spilled):
                    body, got, improved = cut, after, True
            if improved:
                continue
            # A cheap value with an independently reproducible definition is
            # not a stack object.  Recreate those first, then ask allocation
            # again before committing any of the values they conflicted with
            # to frame slots.  Otherwise one rematerializable selector and
            # the short selector ranges it crosses are all spilled together,
            # even though removing the former makes every latter range fit.
            rematerializable = spiller.rematerializable(body, got.spilled)
            if rematerializable:
                remade, made = spiller.spilled(body, rematerializable, self.frame)
                if remade != body:
                    body = remade
                    reloads |= made
                    continue
            # Classification names a candidate, not evidence that rewriting
            # it changed the body. Retrying a no-op consumed all twelve
            # allocation rounds in OIMAD while the real spill set was never
            # touched. Rematerialization is a preference, not progress; when
            # it made no structural change, spill the values allocation
            # actually selected in this same round.
            chosen = got.spilled | spiller.siblings(
                body, got.spilled, self.frame, frozenset(self.pinned) | reloads | retained
            )
            body, made = spiller.spilled(body, chosen, self.frame)
            reloads |= made
        self.pinned = {**prefer, **constrain.required(body)}
        return applied(body, assigned(body))


def _sibling_priced(body: lir.LirBody, live: dict) -> dict:
    """Intervals whose copies to a value they could share a slot with cost nothing.

    Spilling a value spills the copies between it and its non-interfering
    siblings with it, and `spiller.siblings` then puts them in one slot
    where the copy writes a cell from itself. Priced as references, those
    copies made nbody's accY -- a phi, a sum and three copies -- dearer to
    spill than the loop counter, and the counter went to memory instead.

    Only between two values that are nothing but moves and updates in
    place. A value that is also read -- PLASMA's `fuh` pointer, dereferenced
    and compared on every pass -- pays a reload for each of those reads, and
    discounting its copies spilled it in place of the invariant it was
    being added to.
    """
    from qbopt.backend import spiller
    from qbopt.backend import coalesce

    moves = [
        (block.at, pair)
        for block in body.blocks
        for one in block.insns
        if (pair := spiller._plain_move(one)) is not None and pair[0] != pair[1]
    ]
    if not moves:
        return live
    impure: set[int] = set()
    for one in body.insns:
        if spiller._plain_move(one) is not None:
            continue
        what = one.what
        in_place = (
            what is not None
            and what.op in (ir.Operation.BINARY, ir.Operation.UNARY)
            and not one.requires
            and not one.delivers
            and not any(isinstance(x, ir.Mem) for x in (*what.dests, *what.sources))
        )
        impure.update(
            value
            for value in (*one.defines, *one.uses)
            if not (in_place and value in one.defines and value in one.uses)
        )
    near = coalesce._interference(body)
    deep = ranges.depths(body)
    free: dict[int, float] = {}
    for at, (into, out_of) in moves:
        if into in impure or out_of in impure or out_of in near.get(into, ()):
            continue
        each = float(ranges.PER_LEVEL ** deep.get(at, 0))
        for value in (into, out_of):
            free[value] = free.get(value, 0.0) + each
    return {
        value: replace(one, weight=max(0.0, one.weight - free[value] / (one.size + ranges.GRACE)))
        if value in free
        else one
        for value, one in live.items()
    }


def _traffic(body: lir.LirBody, spilled: "frozenset[int]") -> float:
    """The memory references spilling these values costs, weighted by loop depth."""
    deep = ranges.depths(body)
    return sum(
        float(ranges.PER_LEVEL ** deep.get(block.at, 0))
        for block in body.blocks
        for one in block.insns
        for value in (*one.defines, *one.uses)
        if value in spilled
    )


def _retainable_bases(body: lir.LirBody, spilled: "frozenset[int]") -> frozenset[int]:
    """Spilled invariant frame loads that repeatedly form hot addresses.

    A stable frame load ordinarily rematerializes cheaply, which is exactly
    right for an occasional use.  It is wrong for a loop-invariant pointer
    whose only useful role is as the base of a memory operand in a hot block:
    every rematerialization then reconstructs the same address.  This names
    the semantic shape, not a procedure, source register, or physical
    register; allocation still chooses a legal member of the address class.
    """
    if not spilled:
        return frozenset()
    from qbopt.backend import spiller

    stable = frozenset(spiller._stable_loads(body, spilled))
    if not stable:
        return frozenset()
    deep = ranges.depths(body)
    hot_bases = {
        where.base.value
        for block in body.blocks
        if deep.get(block.at, 0) > 0
        for one in block.insns
        if one.what is not None
        for where in (*one.what.dests, *one.what.sources)
        if isinstance(where, ir.Mem) and isinstance(where.base, ir.Held)
    }
    return stable & hot_bases


def _scoped_foldable_indexes(body: lir.LirBody, bases: frozenset[int]) -> frozenset[int]:
    """Dying word indexes in a natural loop that actually uses ``bases``.

    This is deliberately narrower than every foldable index in the body.
    A pre-allocation fold is part of an evaluated loop pressure plan only
    when its loop contains one of that plan's freshly scoped owners.  The
    general legality proof remains in ``spiller.foldable_indexes``.
    """
    if not bases:
        return frozenset()
    from qbopt.analysis import loops

    blocks = {block.at: block for block in body.blocks}
    indexes: set[int] = set()
    for loop in loops.loops(body.blocks, body.entry):
        inside = (blocks[at] for at in loop.body)
        cells = [
            where
            for block in inside
            for one in block.insns
            if one.what is not None
            for where in (*one.what.dests, *one.what.sources)
            if isinstance(where, ir.Mem)
        ]
        if not any(isinstance(where.base, ir.Held) and where.base.value in bases for where in cells):
            continue
        indexes.update(
            where.index.value
            for where in cells
            if isinstance(where.index, ir.Held) and where.scale == 1
        )
    from qbopt.backend import spiller

    return spiller.foldable_indexes(body, frozenset(indexes))


def _fold_discount(one: lir.Insn, profile: targets.Profile) -> float:
    """How much of a spilled read disappears when it becomes a memory operand.

    Interval weights count a memory read as one unit.  Folding removes the
    standalone load, but the memory form may itself cost more than its
    register form.  Price that remainder against the load it replaced.  A
    386 ``add reg,[mem]`` is four cycles dearer than ``add reg,reg`` and a
    load is four cycles, so the read is not free.  Core prices both ALU forms
    equally, so the complete read cost disappears.

    Only forms explicitly present in the immutable CPU profile participate.
    An unpriced encoding is retained at full cost rather than guessed.
    """
    if one.what is None:
        return 0.0
    match one.what:
        case ir.Semantics(ir.Operation.BINARY | ir.Operation.COMPARE, name, _, _):
            if name not in {"add", "sub", "and", "or", "xor", "cmp"}:
                return 0.0
            register, memory = "alu_rr", "alu_rm"
        case ir.Semantics(
            ir.Operation.MULTIPLY,
            "imul",
            _,
            (ir.Held(width=4), ir.Held(width=4)),
        ):
            register, memory = "imul_r32", "imul_m32"
        case _:
            return 0.0
    if not all(profile.prices(form) for form in (register, memory, "mov_rm")):
        return 0.0
    load = profile.cost("mov_rm")
    if load <= 0:
        return 0.0
    remainder = max(0, profile.cost(memory) - profile.cost(register))
    return max(0.0, min(1.0, 1.0 - remainder / load))


def _fold_priced(body: lir.LirBody, live: dict, profile: targets.Profile) -> dict:
    """Discount reads by the target-specific saving from folding them.

    On a target where ``cmp si,[limit]`` costs the same as the register form,
    the read is no reason to keep the limit in a register.  On an older
    target the memory-form premium may retain some or all of that reason.
    Priced uniformly, PLASMA's inner limit, compared once per pixel, outbid
    the outer counter it was computed from, and the counter was added to and
    reloaded in memory instead.
    """
    from qbopt.backend import spiller

    deep = ranges.depths(body)
    free: dict[int, float] = {}
    for block in body.blocks:
        each = float(ranges.PER_LEVEL ** deep.get(block.at, 0))
        for one in block.insns:
            discount = _fold_discount(one, profile)
            if discount == 0.0:
                continue
            for value in one.uses:
                if spiller.folded_source(one, frozenset({value})) is not None:
                    free[value] = free.get(value, 0.0) + each * discount
    return {
        value: replace(one, weight=max(0.0, one.weight - free[value] / (one.size + ranges.GRACE)))
        if value in free and one.weight != float("inf")
        else one
        for value, one in live.items()
    }


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
            f"at a cost of {got.cost:g} and spilling them did not settle. Measured on bools-q-evt: the "
            f"same twelve every round, thirty-six instructions added each time. Their ranges cross calls "
            f"that clobber every register, so no register can hold them and the reload cannot either"
        )
    body = _dead_insertions(body)
    held = got.where
    # An identity copy is dropped here, which is what LLVM's
    # VirtRegRewriter does: `mov ax,ax` is what a split or a phi's copy
    # becomes when both halves land in the same register, and select emits
    # nothing for it -- an instruction of zero length, which the length
    # accounting then disagrees with itself about.
    return replace(
        body,
        blocks=tuple(
            lir.LirBlock(
                at=block.at,
                # `lir.without` rather than a filter: a copy this drops may
                # stand for bytes BC wrote, and layout refuses a body it
                # cannot account for every one of -- matrix and nested
                # refused with `2 bytes between the ops are not
                # instructions` at a `mov bx,ax`.
                insns=tuple(
                    _identity_anchor(one)
                    for one in lir.without(
                        block.insns,
                        _discardable_identity,
                        lambda one: _placed_for_rewrite(one, held, body.origin),
                    )
                ),
                succ=block.succ,
                phis=block.phis,
            )
            for block in body.blocks
        ),
        origin=body.origin,
        pins=body.pins,
        ordered=body.ordered,
    )


def _dead_insertions(body: lir.LirBody) -> lir.LirBody:
    """Delete unused allocator copies before physical identity loses their use graph."""
    if any(one.what is None or one.what.op is ir.Operation.BARRIER for one in body.insns):
        return body
    while True:
        used = {value for one in body.insns for value in one.uses}
        used.update(held.value for one in body.insns for held, _ in one.requires)
        used.update(value for block in body.blocks for phi in block.phis for _, value in phi.incoming)
        dead = set()
        for one in body.insns:
            if (
                not one.covers
                or one.covers[0] != one.covers[1]
                or one.spread
                or one.clobbers
                or one.requires
                or one.delivers
                or one.symbol is True
            ):
                continue
            match one.what:
                case ir.Semantics(ir.Operation.MOVE, "mov", (ir.Held(value, _),), (source,)):
                    if (
                        one.defines == (value,)
                        and value not in used
                        and (isinstance(source, (ir.Held, ir.Imm)) or isinstance(source, ir.Mem) and one.spill_reload)
                    ):
                        dead.add(id(one))
        if not dead:
            return body
        body = replace(
            body,
            blocks=tuple(
                replace(block, insns=tuple(one for one in block.insns if id(one) not in dead)) for block in body.blocks
            ),
        )


def _identity_anchor(one: lir.Insn) -> lir.Insn:
    """Retain byte ownership without requiring an encodable register self-copy."""
    if one.group is not None or not _pointless(one):
        return one
    return replace(one, what=ir.Semantics(ir.Operation.NOTHING, "nop", (), ()))


def _discardable_identity(one: lir.Insn) -> bool:
    """An identity that is not still owned by the parallel-copy scheduler."""
    return one.group is None and _pointless(one)


def _placed_for_rewrite(one: lir.Insn, held: dict, origin: dict) -> lir.Insn:
    """Place one instruction without discarding an inserted definition.

    A copy can become an identity after placement.  It emits no instruction,
    but its definition still explains the value named by later LIR.  Turn it
    into a zero-cost semantic marker before ``lir.without`` decides which
    machine copies can disappear.  This applies equally to inserted copies and
    copies that own original bytes; the marker retains byte ownership while its
    now-absent symbolic operand no longer owns a relocation.
    """
    placed = _placed(one, held, origin)
    if placed.group is None and _pointless(placed):
        return lir.anchor(placed)
    return placed


def _pointless(one: lir.Insn) -> bool:
    """Whether this instruction moves a register into itself."""
    what = one.what
    if what is None or what.op is not ir.Operation.MOVE:
        return False
    if len(what.dests) != 1 or len(what.sources) != 1:
        return False
    into, out_of = what.dests[0], what.sources[0]
    return isinstance(into, ir.Reg) and isinstance(out_of, ir.Reg) and into.register == out_of.register


def _placed(one: lir.Insn, held: dict, origin: dict) -> lir.Insn:
    if one.what is None:
        return one
    what = one.what
    dests = tuple(_settled(x, held, origin) for x in what.dests)
    sources = tuple(_settled(x, held, origin) for x in what.sources)
    # Unconditionally, with no "nothing changed" shortcut: `Mem.through` is
    # `compare=False`, so a cell that just gained the register its base was
    # given compares equal to the one without it, and the shortcut returned
    # the unresolved instruction it had already replaced.
    return replace(one, what=replace(what, dests=dests, sources=sources))


def _settled(where: ir.Loc | ir.Held, held: dict, origin: dict) -> ir.Loc:
    """One operand with its value resolved to the register holding it."""
    if isinstance(where, ir.Mem) and where.selector is not None:
        register = held.get(where.selector.value)
        if register is None:
            raise Unplaced(f"selector value#{where.selector.value} has no register")
        where = replace(where, addr=replace(where.addr, segment=register))
    if isinstance(where, ir.Mem) and where.index is not None:
        base = held.get(where.base.value) if where.base is not None else None
        index = held.get(where.index.value)
        if index is None or (where.base is not None and base is None):
            raise Unplaced(f"scaled cell {where} has no register for its base or index")
        return replace(
            where,
            through=target.named(base, where.base.width) if base is not None else where.through,
            index_through=target.named(index, where.index.width),
        )
    if isinstance(where, ir.Mem) and where.base is not None:
        # The cell keeps saying which value reached it; `through` becomes
        # the register that value was given. Everything else is untouched.
        register = held.get(where.base.value)
        if register is None:
            return where
        return replace(where, through=target.named(register, 2))
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
