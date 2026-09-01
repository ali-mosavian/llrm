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
from qbopt import flags
from qbopt.module import Addr
from qbopt.module import Space
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


# The passes, in the order they run. One per whole-segment round, because
# each round re-raises the body from what the last one wrote -- an op's
# defines and uses are computed at raise time, so a pass that has already
# rewritten the op list is describing the body that went in, not the one
# that came out.
#
# Running them one at a time is what makes that ordering merely an order
# rather than a correctness argument. It was the latter: widening ran
# before avail.py once and avail forwarded a stale high half across an op
# that said it read two bytes where the instruction read four. That is no
# longer possible to get wrong by rearranging this list.
PASSES = ("drop_loads", "drop_stores", "widen", "place", "absorb")


def applied(
    body: MirBody,
    dgroup: frozenset[int],
    calls: dict[int, str],
    *,
    blocks: list | None = None,
    absorb: bool = False,
    found=None,
    widen: bool = True,
    drop_loads: bool = True,
    drop_stores: bool = True,
    place: bool = False,
    only: str | None = None,
) -> MirBody:
    """Every transform this module has, or the one `only` names.

    `absorb` is off by default and not because it is unsound: calls.py has
    already taken every arithmetic call before a body reaches here, so with
    it on the MIR emitter finds nothing and no gate exercises it.
    `--no-absorb-calls` is the lever that makes the two comparable.

    `place` is off for its own reason: sinking a definition is the only
    transform here that changes the order instructions run in, and its
    benefit is indirect.
    """
    wanted = {
        "drop_loads": drop_loads,
        "drop_stores": drop_stores,
        "widen": widen,
        "place": place,
        "absorb": absorb and blocks is not None,
    }
    for name in PASSES:
        if only is not None and name != only:
            continue
        if not wanted[name]:
            continue
        if name == "drop_loads":
            body = without_redundant_loads(body, dgroup, calls)
        elif name == "drop_stores":
            body = without_dead_stores(body, dgroup, calls)
        elif name == "widen":
            body = widened(body)
        elif name == "place":
            body = placed(body)
        elif name == "absorb":
            body = absorbed(body, blocks, calls, found)
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

EMITTED = ("B$MUI4", "B$DVI4", "B$RMI4", "B$CPI4")

# B$CPI4 rebuilds its answer through lahf/sahf because an 8086 cannot
# compare a long in one go. A 386 can, and the flags a `cmp` leaves are the
# ones the following jcc wants -- but only the signed and equality ones. CF,
# PF and AF are the runtime's own synthesis rather than the comparison's, so
# a site that reads one of them afterwards is left alone. calls.py's own
# note above its RESULT says the rest, including why this pass's answer is
# the *right* one on operands where the runtime's is not.
SYNTHESISED = flags.Flag.CF | flags.Flag.PF | flags.Flag.AF


def _wide(register: "Register") -> ir.Reg:
    return ir.Reg(register=register, width=4)


def _narrow(register: "Register") -> ir.Reg:
    return ir.Reg(register=register, width=2)


def _frame(disp: int, width: int) -> ir.Mem:
    """`[bp+disp]`, which is the only base 16-bit addressing has to offer."""
    return ir.Mem(Addr(Space.FRAME, disp), width)


def _flags_after(blocks: list, live: dict, at: int, end: int) -> "flags.Flag":
    """The flags something reads after this region, or all of them if unknown.

    flags.py's own analysis, not a second one. It is a block-level walk over
    instructions -- the same kind of thing stack.py is, and no more a
    machine-code rewrite than that. What M5 retires is the emission.
    """
    block = next((one for one in blocks if one.at <= at < one.end), None)
    return flags.live_after(block, end, live) if block is not None else flags.ALL


def _comparing(at: int) -> list[ir.Semantics]:
    """A long compare, with both arguments still on the stack.

    B$CPI4 changes no register at all -- calls.py's note above its RESULT
    reads that out of the runtime source -- so absorbing it must not either,
    and a `cmp` of two stack cells needs a register for one side. bp is the
    only base 16-bit addressing can use with a displacement, so it stands in
    as a frame pointer just long enough to name both arguments in place, and
    edx holds the left one. Both are put back.

    The saved bp cannot simply be read back from where `push` left it: that
    slot is below sp the moment sp is raised past it, and DOS services
    interrupts at any instruction boundary onto whatever stack is live. So
    it is read before sp moves at all, parked in the call's own dead
    argument space, and only the last `pop` ever reads below where sp
    already sits.

    Everything after the `cmp` has to leave the flags alone, which is why
    the moves are moves and the stack is raised with `lea` rather than
    `add sp`.
    """
    def made(op: ir.Operation, name: str, dests, sources) -> ir.Semantics:
        return ir.Semantics(op, name, dests=tuple(dests), sources=tuple(sources))

    edx, bp, sp, dx = _wide(Register.EDX), _narrow(Register.BP), _narrow(Register.SP), _narrow(Register.DX)
    return [
        made(ir.Operation.PUSH, "push", (), (bp,)),
        made(ir.Operation.PUSH, "push", (), (edx,)),
        made(ir.Operation.MOVE, "mov", (bp,), (sp,)),
        # six bytes pushed ahead of the arguments puts the left one -- the
        # deeper, since B$CPI4 takes it first -- at +10, and the right at +6
        made(ir.Operation.MOVE, "mov", (edx,), (_frame(10, 4),)),
        made(ir.Operation.COMPARE, "cmp", (), (edx, _frame(6, 4))),
        # from here on the flags are the answer and nothing may write them
        made(ir.Operation.MOVE, "mov", (dx,), (_frame(4, 2),)),
        # +12 is the top two bytes of the left argument, already read into
        # edx above, and still above where sp ends up
        made(ir.Operation.MOVE, "mov", (_frame(12, 2),), (dx,)),
        made(ir.Operation.MOVE, "mov", (edx,), (_frame(0, 4),)),
        made(ir.Operation.ADDRESS, "lea", (sp,), (ir.Address(Addr(Space.FRAME, 12)),)),
        made(ir.Operation.POP, "pop", (bp,), ()),
    ]


def _from(operand) -> ir.Loc:
    """One classified operand as somewhere an instruction can read it."""
    from qbopt.calls import Kind, relocated_addr

    if operand.kind is Kind.CONSTANT:
        return ir.Imm(value=operand.value, width=4)
    return ir.Mem(relocated_addr(operand), 4)


def _deleting(site, ops_at: dict) -> list[ir.Semantics] | None:
    """A call whose operands have addresses, reloaded rather than popped.

    calls.py's own strategy for 997 of the corpus's 1,151 sites, and the one
    that makes absorption smaller than what BC wrote rather than larger: the
    pushes go too, so the region is push-through-call and four bytes of
    stack traffic per operand disappear with it. Popping keeps them, which
    is why MIR-only absorption came out 19,435 bytes *above* BC.

    A comparison wraps eax in push/pop. B$CPI4 changes no register at all
    and BC's own code can be relying on that anywhere around the call, not
    only in the flags -- and `pop` does not touch the ones the `cmp` set.
    """
    def made(op, name, dests, sources, field=None):
        return ir.Semantics(op, name, dests=tuple(dests), sources=tuple(sources)), field

    name = site.name.upper()
    left, right = site.operands
    eax, edx = _wide(Register.EAX), _wide(Register.EDX)
    steps: list = []

    if name == "B$CPI4":
        steps.append(made(ir.Operation.PUSH, "push", (), (eax,)))
    steps.append(made(ir.Operation.MOVE, "mov", (eax,), (_from(left),), left.at))

    other = _from(right)
    if name == "B$MUI4":
        steps.append(made(ir.Operation.MULTIPLY, "imul", (eax,), (eax, other), right.at))
    elif name == "B$CPI4":
        steps.append(made(ir.Operation.COMPARE, "cmp", (), (eax, other), right.at))
    else:
        steps.append(made(ir.Operation.EXTEND, "cdq", (edx,), (eax,)))
        if isinstance(other, ir.Imm):
            return None  # idiv has no immediate form and nothing loads one here
        steps.append(made(ir.Operation.DIVIDE, "idiv", (eax, edx), (eax, edx, other), right.at))
        if name == "B$RMI4":
            steps.append(made(ir.Operation.MOVE, "mov", (eax,), (edx,)))

    if name == "B$CPI4":
        steps.append(made(ir.Operation.POP, "pop", (eax,), ()))
    return steps


def _laid_at(steps: list, here: dict, lo: int, hi: int) -> list[Op] | None:
    """One site's operations as ops, all on the region's first address.

    A step that reads a relocated address is built from the push that
    carried it, so its node still spans the fixup and layout.py finds it
    where it always did. Everything else carries no node at all: its own
    address is inside a far call whose target is a fixup too, and a search
    would find that one.
    """
    out: list[Op] = []
    for number, (what, field) in enumerate(steps):
        carrier = None
        if field is not None:
            carrier = next(
                (one for one in here.values()
                 if one.node is not None and ir.span(one.node)[0] <= field < ir.span(one.node)[1]),
                None,
            )
            if carrier is None:
                return None
        seed = carrier if carrier is not None else next(iter(here.values()))
        out.append(
            replace(
                seed,
                at=lo,
                op=what.op,
                name=what.name or "",
                defines=(),
                uses=(),
                loads=(),
                stores=(),
                node=seed.node if carrier is not None else None,
                made=what,
                covers=(lo, hi) if number == 0 else (lo, lo),
            )
        )
    return out


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

    if name == "B$CPI4":
        steps = _comparing(at)
        return _laid(steps, at, after, restore=False)

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
        if name == "B$RMI4":
            # idiv leaves the quotient in eax and the remainder in edx, and
            # BC reads either one in dx:ax.
            steps.append(made(ir.Operation.MOVE, "mov", (_wide(RESULT),), (_wide(Register.EDX),)))

    return _laid(steps, at, after, restore=True)


def _laid(steps: list[ir.Semantics], at: int, after: Op, restore: bool) -> list[Op]:
    """One site's operations as ops, all standing on the call's own address."""
    end = after.node.insn.end if after.node is not None else at
    out: list[Op] = []
    for number, what in enumerate(steps):
        out.append(
            replace(
                after,
                at=at,
                op=what.op,
                name=what.name or "",
                defines=(),
                uses=(),
                loads=(),
                stores=(),
                node=None,
                made=what,
                # All on the call's own address, and only the first standing
                # for its bytes. layout.py places by position and keys the
                # address map by the first of a group, so the order these
                # are listed in is the order they run in, and a branch to
                # the call arrives at the start of what replaced it.
                covers=(at, end) if number == 0 else (at, at),
            )
        )
    # `push eax / pop ax / pop dx` -- BC reads a long in dx:ax and the
    # arithmetic left it in eax. pairs.py's own restore, for the same
    # reason. A comparison has no result to hand back and takes none.
    if restore:
        out.append(pairs._restore_op(0, at, out[-1], at))
    return out


def absorbed(body: MirBody, blocks: list, calls: dict[int, str], found=None) -> MirBody:
    """Every arithmetic runtime call this can compute in place, computed.

    Two strategies, chosen per site and never mixed, which is calls.py's own
    split. **Delete**: every operand has an address or is an immediate, so
    it is reloaded at codegen time and the pushes go with the call -- 997 of
    the corpus's 1,151 sites, and the reason absorption is smaller than what
    BC wrote. **Consume**: something is only on the stack, so every byte is
    popped wherever it actually sits; reloading one operand and leaving its
    push standing would leak four bytes of stack per call, forever.

    Refused on the flags, and on which flags. The three arithmetic routines
    return a value and leave the flags incidental, so any read of them after
    the site refuses it: `imul` and `idiv` write their own. A comparison's
    flags *are* its result, so the question is narrower -- CF, PF and AF are
    the runtime's own synthesis and a `cmp` does not reproduce them.

    That is flags.py's analysis rather than MIR's own values, and
    deliberately: MIR has one FLAGS pseudo-register and cannot say which
    flag, which for the comparison is the whole question.
    """
    from qbopt import calls as machine

    if found is None:
        return body
    reached = [one for block in blocks for one in block.insns]
    sites = {one.at: one for one in machine.sites(found, reached, blocks)}
    if not sites:
        return body

    live = flags.live_in(blocks)
    out = []
    for block in body.blocks:
        ops: list[Op] = []
        drop: set[int] = set()
        for op in block.ops:
            if op.at in drop:
                continue
            site = sites.get(op.at)
            name = (calls.get(op.at) or "").upper()
            if site is None or name not in EMITTED or getattr(op.node, "insn", None) is None:
                ops.append(op)
                continue
            read = _flags_after(blocks, live, site.start, site.end)
            wrong = SYNTHESISED if name == "B$CPI4" else flags.ALL
            if read & wrong:
                ops.append(op)
                continue
            if site.consume:
                ops.extend(_absorbing(name, op.at, op))
                continue
            steps = _deleting(site, {})
            here = {one.at: one for one in block.ops if site.start <= one.at < site.end}
            built = _laid_at(steps, here, site.start, site.end) if steps else None
            if built is None:
                ops.append(op)
                continue
            # the pushes go with the call, which is what makes this smaller
            drop.update(here)
            ops = [one for one in ops if not (site.start <= one.at < site.end)]
            if name != "B$CPI4":
                # the arithmetic leaves its answer in eax and BC reads a long
                # in dx:ax; a comparison's answer is flags and takes none
                built.append(pairs._restore_op(0, site.start, built[-1], site.start))
            ops.extend(built)
        out.append(replace(block, ops=tuple(ops)))
    return replace(body, blocks=tuple(out))
