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
from qbopt import pairs
from qbopt import layout
from qbopt.mir import Op
from qbopt.declen import Insn
from iced_x86 import Register
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


def widened(body: MirBody, dead: frozenset[int] = frozenset()) -> MirBody:
    """Every chain worth widening, as 32-bit operations on one register.

    This was wrong once and is worth saying how, because the fix was not the
    part that looked wrong. It renamed `add ax,[x]` with `adc dx,[x+2]` to
    `add eax,[x]`, and BC keeps that long in dx:ax -- so the carry landed in
    eax's high half and dx kept what it held. The rename itself is right;
    what was missing either side of it is:

    - the **chain**. One pair widened in isolation says nothing about where
      the long came from or goes. qbopt/pairs.py answers that -- six of its
      shapes agree with lift.py exactly, object by object.
    - the **restore**. `push eax / pop ax / pop dx` hands the long back to
      BC's sixteen-bit code, and without it every later read of dx is stale.
    - the **cost**. Two instructions become one plus a four-byte restore, so
      a lone pair widened is longer than what BC wrote. 235 of qb-qrender's
      341 chains would grow.

    All three live in pairs.py; this is where they are applied.
    """
    return pairs.widened(body, dead)


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
    blocks: list | None = None,
    absorb: bool = False,
    widen: bool = True,
    drop_loads: bool = True,
    drop_stores: bool = True,
    place: bool = False,
) -> MirBody:
    """Every transform this module has, in the order they help each other.

    `absorb` is off by default and not because it is unsound: calls.py has
    already taken every arithmetic call before a body reaches here, so with
    it on the MIR emitter finds nothing and no gate exercises it.
    `--no-absorb-calls` is the lever that makes the two comparable.
    """
    if drop_loads:
        body = without_redundant_loads(body, dgroup, calls)
    if drop_stores:
        body = without_dead_stores(body, dgroup, calls)
    # Widening last. A widened op keeps the low half's own `loads` and
    # `stores` -- two bytes at [x] -- while the instruction reads four, so
    # avail.py asked whether [x+2] had been written and was told nothing
    # had touched it. Running it after the passes that reason about memory
    # means none of them ever sees the mismatch. It cost the reverse
    # ordering's claimed benefit, which was that folding a pair makes a
    # later reload of the same cell visible as redundant rather than as the
    # high half's own read -- worth having, and not at this price.
    if widen:
        body = widened(body)
    # Off by default. Sinking a definition is the only transform here that
    # changes the order instructions run in, and its own benefit is
    # indirect -- shorter live ranges, which other passes then use. It gets
    # switched on when something asks for that, not before.
    if place:
        body = placed(body)
    # Absorption last, and needing the blocks: `arguments()` reads a stack
    # depth off the instructions, and a transform that has already replaced
    # some of them is not what that model was measured against.
    if absorb and blocks is not None:
        body = absorbed(body, blocks, calls)
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


def _absorbable(name: str | None) -> int | None:
    """How many long arguments this routine takes, or None if it is not one
    absorption knows.

    stack.frames() asks this to decide whether a call's own gap can be
    trusted -- that it consumed exactly this many longs and returned with
    nothing else disturbed. The claim rests on the QuickBASIC 4.5 runtime
    source, where these four are callee-cleanup and clobber only ax, cx, dx
    and bx.
    """
    return 2 if name is not None and name.upper() in ABSORB else None


def arguments(blocks: list, calls: dict[int, str]) -> dict[int, tuple[tuple[Insn, ...], ...]]:
    """Each absorbable call's two long operands, as the pushes that put them
    there, left operand first.

    Built on `stack.frames()` rather than beside it. This used to walk the
    MIR ops keeping a depth of its own, and named 84 of the corpus's 1,151
    sites while getting all 84 wrong: an unrecognised call ended the block's
    depth and 1,017 sites sit after one; a `push word [x]` was invisible,
    because it only recorded a push with exactly one register use, and 5,492
    of the corpus's 8,401 pushes are that shape; and it counted stack slots
    where a long is two of them, so the pair it returned was the two halves
    of one operand rather than the two operands.

    `stack.py` answers all three already and `calls.grouped()` already
    splits a frame into arguments. Neither is a machine-code rewrite -- one
    is a depth model over a block's instructions, the other byte arithmetic
    over pushes. What M5 retires is the emission, not the analysis under it.
    """
    from qbopt import stack
    from qbopt.calls import grouped

    found: dict[int, tuple[tuple[Insn, ...], ...]] = {}
    for block in blocks:
        for frame in stack.frames(block, calls, _absorbable):
            name = (calls.get(frame.call.at) or "").upper()
            groups = grouped(frame.pushed)
            if groups is None or len(groups) != 2:
                continue
            left, right = groups if LEFT_FIRST[name] else (groups[1], groups[0])
            found[frame.call.at] = (tuple(left), tuple(right))
    return found


# The register each popped operand lands in, left first -- the dividend and
# the multiplicand in eax, the divisor and multiplier in ecx. `idiv` names
# only the divisor: its dividend is edx:eax and that is what `cdq` widens
# eax into.
INTO = (Register.EAX, Register.ECX)

# What the result comes back in, which is what BC reads as dx:ax.
RESULT = Register.EAX

# A far call is five bytes, so a site has five addresses to put operations
# on -- and layout.py keys every operation by one. Multiply needs four and
# divide five; the remainder needs six, because its answer comes back in edx
# and has to be moved. That is why B$RMI4 is not here, and it is an address
# budget rather than anything about the arithmetic.
EMITTED = {"B$MUI4": 4, "B$DVI4": 5}


def _wide(register: "Register") -> ir.Reg:
    return ir.Reg(register=register, width=4)


def _read_flags(body: MirBody) -> frozenset:
    """Every flags value something in this body reads.

    A call is not one of them. MIR gives every call a use of FLAGS, which is
    the conservative answer for a body that could branch on them -- and it
    makes every absorbable site look like its flags are read, because the
    next call reads them. Nothing in BC's convention passes a value in
    flags and no runtime routine branches on the caller's, which is the
    claim calls.py's own flag liveness has always rested on.

    A flags value reaching a phi still counts: it is live out of the block
    and what reads it is not visible from here.
    """
    seen = set()
    for block in body.blocks:
        for phi in block.phis:
            seen.update(one for one in phi.incoming.values() if one.flags)
        for op in block.ops:
            if op.op is ir.Operation.CALL:
                continue
            seen.update(one for one in op.uses if one.flags)
    return frozenset(seen)


def _absorbing(name: str, at: int, after: Op) -> list[Op]:
    """The operations one absorbed call becomes, in order.

    The arguments are popped rather than reloaded, which is what makes this
    sound at a site whose pushes are not contiguous: `stack.frames()` proves
    the four bytes of each operand are the topmost region of the stack, and
    popping them takes exactly what a real callee-cleanup call would have.
    Reloading one operand from its address instead would leave its push
    standing and leak four bytes of stack per call, forever.

    The pushes are left where they are. Only the call is replaced, so
    nothing between them has to be accounted for and the region is five
    bytes wide however far apart they were pushed.
    """
    def made(op: ir.Operation, name: str, dests, sources) -> ir.Semantics:
        return ir.Semantics(op, name, dests=tuple(dests), sources=tuple(sources))

    steps: list[ir.Semantics] = [
        made(ir.Operation.POP, "pop", (_wide(INTO[0]),), ()),
        made(ir.Operation.POP, "pop", (_wide(INTO[1]),), ()),
    ]
    if name == "B$MUI4":
        steps.append(
            made(ir.Operation.MULTIPLY, "imul", (_wide(RESULT),), (_wide(RESULT), _wide(INTO[1])))
        )
    else:
        # cdq, not cwd: the dividend is the whole 32 bits of eax and idiv
        # reads edx:eax, so the sign has to reach edx or every negative
        # dividend divides as if it were huge and positive.
        steps.append(made(ir.Operation.EXTEND, "cdq", (_wide(Register.EDX),), (_wide(RESULT),)))
        steps.append(
            made(
                ir.Operation.DIVIDE,
                "idiv",
                (_wide(RESULT), _wide(Register.EDX)),
                (_wide(RESULT), _wide(Register.EDX), _wide(INTO[1])),
            )
        )

    out: list[Op] = []
    for number, what in enumerate(steps):
        out.append(
            replace(
                after,
                at=at + number,
                op=what.op,
                name=what.name or "",
                defines=(),
                uses=(),
                loads=(),
                stores=(),
                node=None,
                made=what,
                # The first stands for the call's own five bytes and the
                # rest for none, which is layout.py's own arithmetic for a
                # transform that puts more operations where fewer were.
                covers=(at, after.node.insn.end if after.node is not None else at) if number == 0 else (at + number, at + number),
            )
        )
    # `push eax / pop ax / pop dx` -- BC reads a long in dx:ax and the
    # arithmetic left it in eax. pairs.py's own restore, for the same reason.
    out.append(pairs._restore_op(0, at + len(steps), out[-1], at + len(steps)))
    return out


def absorbed(body: MirBody, blocks: list, calls: dict[int, str]) -> MirBody:
    """Every arithmetic runtime call this can compute in place, computed.

    The half of M5 that is emission rather than analysis. `arguments()` says
    where a call's operands are; this says what the call becomes.

    Refused where anything reads the flags the call defined: `imul` and
    `idiv` leave their own, and a `jcc` after the site would read a
    different answer. Refused too where the operands are not both four bytes
    on top of the stack, which is `stack.frames()`' own claim.
    """
    where = arguments(blocks, calls)
    if not where:
        return body

    read = _read_flags(body)
    out = []
    for block in body.blocks:
        ops: list[Op] = []
        for op in block.ops:
            name = (calls.get(op.at) or "").upper()
            if op.at not in where or name not in EMITTED or op.node is None:
                ops.append(op)
                continue
            if any(one.flags and one in read for one in op.defines):
                ops.append(op)
                continue
            if getattr(op.node, "insn", None) is None:
                ops.append(op)
                continue
            ops.extend(_absorbing(name, op.at, op))
        out.append(replace(block, ops=tuple(ops)))
    return replace(body, blocks=tuple(out))
