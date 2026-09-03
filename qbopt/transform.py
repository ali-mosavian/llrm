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

from collections.abc import Iterator
from itertools import product
from dataclasses import replace

from qbopt import consts
from qbopt import ir
from qbopt import lower
from qbopt import lir
from qbopt import mir
from qbopt import module
from qbopt import wide
from qbopt.passes import MIRTransform
from qbopt.passes import Where
from qbopt import avail
from qbopt import runtime
from qbopt import regalloc
from qbopt import pairs
from qbopt import loops as loopy
from qbopt import layout
from qbopt.mir import Op
from qbopt.declen import Insn
from qbopt import flags
from qbopt.module import Addr
from qbopt.module import Space
from iced_x86 import Register
from iced_x86 import Register_
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


# A root register at the width an operand reads it. ir.ROOT maps the narrow
# name to the wide one; this is the way back, and only for the general
# registers -- a segment register has no narrower form and is never a
# provider here.
_AT_WIDTH = {
    Register.EAX: {4: Register.EAX, 2: Register.AX, 1: Register.AL},
    Register.EBX: {4: Register.EBX, 2: Register.BX, 1: Register.BL},
    Register.ECX: {4: Register.ECX, 2: Register.CX, 1: Register.CL},
    Register.EDX: {4: Register.EDX, 2: Register.DX, 1: Register.DL},
    Register.ESI: {4: Register.ESI, 2: Register.SI},
    Register.EDI: {4: Register.EDI, 2: Register.DI},
    Register.EBP: {4: Register.EBP, 2: Register.BP},
}


def _at_width(register, width: int):
    return _AT_WIDTH.get(ir.ROOT.get(register, register), {}).get(width)


def forwarded(body: MirBody, dgroup: frozenset[int], calls: dict[int, str]) -> MirBody:
    """Reads a live register can serve, served from it.

    The operand changes and the instruction does not: `add ax,[y]` becomes
    `add ax,si`, so the destination is untouched and nothing downstream is
    rewritten. That is what makes an accumulate safe here -- deleting one
    would throw the arithmetic away, and this keeps it.

    Both halves of the join have to hold and neither is enough: the cell's
    content has to be known, and the value holding it has to still be live
    at the read. BC spills across calls, and the reload after one is real
    work.

    73 of the corpus's reads, where the crude count of "a load of a cell
    just written" is 618 -- the difference is `add [x],ax` followed by
    `mov cx,[x]`, where the cell is written and no register holds the
    result. Serving those means computing in a register and storing once,
    which is a different transform.
    """
    want = frozenset(op.at for block in body.blocks for op in block.ops if op.loads)
    if not want:
        return body
    served = {
        one.at: one.value
        for one in avail.forwardable(body, dgroup, calls, want)
        if one.value is not None
    }
    if not served:
        return body

    out = []
    for block in body.blocks:
        ops: list[Op] = []
        for op in block.ops:
            holder = served.get(op.at)
            args = _served(op, holder) if holder is not None else None
            ops.append(
                op
                if args is None
                else replace(op, args=args, loads=(), uses=op.uses + (holder,))
            )
        out.append(replace(block, ops=tuple(ops)))
    return replace(body, blocks=tuple(out))


def _served(op: Op, holder) -> "tuple[mir.Arg, ...] | None":
    """`op`'s one memory source read from whatever holds `holder` instead.

    A value, not a register: which one holds it is the allocator's answer,
    and naming one here is what rule 5 forbids. This asked
    `_at_width(root, width)` and wrote the register down, which was
    forward.py's 22 machine references in one line.
    """
    cells = [one for one in op.args if isinstance(one, mir.Cell)]
    if len(cells) != 1:
        return None
    cell = cells[0]
    return tuple(
        mir.Held(holder, cell.ref.width) if one is cell else one for one in op.args
    )


SEGMENT_NAMES = frozenset({"es", "fs", "gs"})


def _segment_load(op: Op):
    """(which resource, what it is loaded from) where this op loads one.

    A descriptor lands in a machine resource MIR has no value for, so it
    arrives as mir.Opaque carrying that resource's own name. This read the
    register number out of the instruction.
    """
    if op.kind is not mir.Kind.LOAD or len(op.results) != 1 or len(op.args) != 1:
        return None
    into = op.results[0]
    if not isinstance(into, mir.Opaque) or into.name not in SEGMENT_NAMES:
        return None
    return into.name, op.args[0]


def segments(body: MirBody, dgroup: frozenset[int], calls: dict[int, str]) -> MirBody:
    """A segment register loaded from what it already holds, dropped.

    `mov es,[desc+2]` twice in one statement, because the element appears
    twice in it -- and again on every pass of the loop, from a word written
    once before the loop ran. `segments.py` does this against the machine
    code and finds 42 sites in qb-qrender; nothing in MIR did.

    Cross-block, and it has to be: what a block starts holding is what every
    path into it agrees on, and the reload that matters most arrives over a
    back-edge. Starting from "nothing is held" and growing is the
    conservative direction -- a cycle cannot talk itself into a fact.

    Hoisting the survivor out of the loop is a separate and larger thing,
    and needs code motion between blocks. This only removes the duplicate.
    """
    blocks = {block.at: block for block in body.blocks}
    preds: dict[int, list[int]] = {at: [] for at in blocks}
    for block in body.blocks:
        for successor in block.succ:
            if successor in preds:
                preds[successor].append(block.at)

    def through(block, holding: dict, gone: set[int] | None):
        holding = dict(holding)
        for op in block.ops:
            if op.barrier or op.at in calls:
                holding = {}
                continue
            found = _segment_load(op)
            if found is not None:
                register, source = found
                if holding.get(register) == source:
                    if gone is not None:
                        gone.add(op.at)
                elif source is not None:
                    holding[register] = source
                continue
            # A write through the segment itself cannot be the descriptor
            # it was loaded from. A dynamic array's storage is outside
            # DGROUP -- that is the whole reason it needs a segment -- and
            # the descriptor is a LITERAL or SEGMENT cell reached through
            # ds. Anything else that writes memory, or writes somewhere
            # this cannot name, puts the descriptor back in doubt.
            if any(one.addr is None or one.addr.space is not Space.FAR for one in op.stores):
                holding = {}
        return holding

    exits = {at: {} for at in blocks}
    for _round in range(len(blocks) + 1):
        changing = False
        for at in sorted(blocks):
            entering: dict | None = None
            for previous in preds[at]:
                was = exits[previous]
                entering = dict(was) if entering is None else {
                    k: v for k, v in entering.items() if was.get(k) == v
                }
            got = through(blocks[at], entering or {}, None)
            if got != exits[at]:
                exits[at] = got
                changing = True
        if not changing:
            break

    gone: set[int] = set()
    for at in sorted(blocks):
        entering = None
        for previous in preds[at]:
            was = exits[previous]
            entering = dict(was) if entering is None else {k: v for k, v in entering.items() if was.get(k) == v}
        through(blocks[at], entering or {}, gone)
    if not gone:
        return body
    return replace(
        body,
        blocks=tuple(replace(one, ops=tuple(_absorb(list(one.ops), gone))) for one in body.blocks),
    )


# Operations with a register operand the encoding does not name: the
# one-operand `imul`/`idiv` whose other half is dx:ax, `cwd` and `cdq`, and
# a shift by cl. select.py cannot remap what is not an operand.
_IMPLICIT = (
    ir.Operation.MULTIPLY,
    ir.Operation.DIVIDE,
    ir.Operation.EXTEND,
)


def _implicit(op: Op) -> bool:
    """Whether this operation needs a value in a register it does not name.

    lir says which, and what an instruction requires is the machine's
    business rather than a pass's: this used to answer it here, in a
    predicate a transform consulted to decide whether to give up.
    """
    what = lower.current(op)
    if what is None:
        return True
    return any(need.fixed is not None for need in lir.reads(what).values())


def _semantics_of(op: Op):
    return lower.current(op)


def _preheader(body: MirBody, loop) -> int | None:
    """The block a loop is entered through, where there is exactly one.

    A hoisted operation has to run once before the loop and on every path
    into it, so it goes in the block that dominates the header from outside
    -- and only where there is one of those. Two entries into a loop is a
    header with two outside predecessors, and synthesising a block for it is
    a bigger change than any pass here needs; those loops are refused.
    """
    outside = [
        block.at
        for block in body.blocks
        if loop.header in block.succ and block.at not in loop.body
    ]
    return outside[0] if len(outside) == 1 else None


def _reads(op: Op) -> frozenset[int]:
    """The registers this instruction actually reads.

    Not the same question as `op.uses`. In a 16-bit program under a 32-bit
    register model every narrow write is a read-modify-write: `mov ax,[n]`
    defines eax and uses the eax before it, because the high half survives.
    115 of the corpus's 211 loop operations that read a loop-carried value
    read it only that way, which is why nothing ever looked invariant.

    A register named nowhere in the semantics is preserved, not read. One
    named in a memory operand -- a base, an index -- is read, and an
    operation this cannot read the semantics of reads everything.
    """
    what = _semantics_of(op)
    # Semantics that name no operand say as little as no semantics at all,
    # and mean it just as literally: calls.py's restore idiom is
    # `push eax / pop ax / pop dx`, which reads eax and edx and writes them
    # down nowhere. Reading that as "reads nothing" makes it invariant in
    # any loop, and lngmix hoisted the one that splits its running sum --
    # 1185033780 for 142900. It was hidden behind the guard above, which
    # refuses an operation whose sources are all registers and finds that
    # vacuously true of an operation with no sources.
    if what is None or op.barrier or (not what.sources and not what.dests and op.uses):
        return frozenset(ir.ROOT.values()) | {one for one in ir.ROOT}
    found: set[int] = set()
    for one in (*what.sources, *what.dests):
        if isinstance(one, ir.Reg) and one in what.sources:
            found.add(ir.ROOT.get(one.register, one.register))
        # A destination's own base and index are read to reach it, however
        # the cell itself is only written.
        for where in (getattr(one, "through", None), getattr(one, "index", None)):
            if where is not None:
                found.add(ir.ROOT.get(where, where))
    return frozenset(found)


def _effective(body: MirBody, calls: dict[int, str]) -> set:
    """Every value some instruction reads, rather than merely preserves.

    Transitive through phis, because the chain that matters is: `imul word
    [k]` preserves edx's high half, the latch's phi carries that, and the
    next iteration's imul preserves it again. Nothing reads it, and yet
    every operation in the run appeared to depend on the one before it
    across the back edge.

    Grown from what is definitely read rather than shrunk from everything,
    so a cycle cannot talk itself into being effective.
    """
    wanted: set = set()
    carrying: dict = {}
    for block in body.blocks:
        for phi in block.phis:
            for value in phi.incoming.values():
                carrying.setdefault(phi.result, set()).add(value)
        for op in block.ops:
            read = _reads(op)
            for use in op.uses:
                where = body.origin.get(use)
                if use.flags or op.at in calls or where is None or ir.ROOT.get(where, where) in read:
                    wanted.add(use)
    changing = True
    while changing:
        changing = False
        for result, incoming in carrying.items():
            if result in wanted and not incoming <= wanted:
                wanted |= incoming
                changing = True
    return wanted


def _starts(phis: list) -> set:
    """Values a phi in this loop carries in from somewhere.

    A definition whose value reaches one begins something the loop goes on
    to change, and moving it out means the next pass sees where the last
    one finished.
    """
    return {value for phi in phis for value in phi.incoming.values()}


def _rewritten(ops: list[Op], origin: dict) -> set:
    """Registers more than one operation in the loop writes.

    A definition may only leave a loop if it is the only one of its register
    in there. `mov ax,1` that starts an inner counter reads nothing the
    outer loop writes, so it is invariant by every other test here -- and
    hoisting it means the second pass of the outer loop starts from where
    the inner one left off. segld printed 1030 for 1050, one inner loop
    short.
    """
    seen: dict = {}
    for one in ops:
        for value in one.defines:
            if value.flags:
                continue
            where = origin.get(value)
            if where is not None:
                seen[where] = seen.get(where, 0) + 1
    return {where for where, count in seen.items() if count > 1}


def _invariant_run(
    ops: list[Op],
    carried: set,
    stores: list,
    dgroup: frozenset[int],
    calls: dict[int, str],
    origin: dict,
    bounds: dict | None = None,
    starts: set | None = None,
    readable: set | None = None,
) -> list[Op]:
    """The ops in this loop whose result never changes, in order.

    Grown rather than filtered: an op is invariant when every cell it reads
    is one no store in the loop can reach, and every register it reads was
    defined outside the loop or by an op already in the run. That second
    clause is why this is a fixed point and not a scan.
    """
    if any(one.at in calls or one.barrier for one in ops):
        return []
    made: set = set()
    run: list = []
    twice = _rewritten(ops, origin)
    begins = starts or set()
    changing = True
    while changing:
        changing = False
        for one in ops:
            what = _semantics_of(one)
            if one in run or one.stores or what is None:
                continue
            # A branch is where the loop is. hotlop's latch block held
            # `cmp`, `jle` and `jmp`, all three reading nothing the loop
            # writes, so all three were invariant by the test above and all
            # three left -- and the back edge left with them.
            if one.kind in (mir.Kind.JUMP, mir.Kind.BRANCH):
                continue
            # Nor anything that computes nothing. A register-to-register
            # move is invariant whenever its source is, so hoisting one and
            # putting a split back in its place is churn -- and the split
            # comes back through emission as an ordinary move, is hoisted in
            # turn, and hotlop's loop body became eighteen `mov cx,bx` in
            # two rounds. What this is for is moving work: a load, or an
            # operation over one.
            # Nor anything that computes nothing. A register-to-register
            # move is invariant whenever its source is, so hoisting one and
            # putting a split back in its place is churn -- and the split
            # comes back through emission as an ordinary move, is hoisted in
            # turn, and hotlop's loop body became eighteen `mov cx,bx` in
            # two rounds. What this is for is moving work: a load, or an
            # operation over one.
            #
            # This catches `add bx,ax` too, which is not a move and is real
            # work, and letting it through looks like a plain bug -- press
            # accumulates four invariant products into bx and with the adds
            # refused each product crosses into the loop wanting a register
            # of its own. It is not a bug. A long add is `add` then `adc`,
            # joined by the carry and by nothing this can see, and with the
            # pair split across the loop edge lngmix printed 1185033780 for
            # 142900. Whatever admits the adds has to keep them together.
            if (
                one.kind is mir.Kind.COPY
                and not one.loads
                and not any(use in made for use in one.uses)
            ):
                continue
            # Only definition of its register in the loop, or the loop's own
            # Both, and neither alone. "A phi carries it" refuses harr's
            # `mov si,0`, which is safe: si is the array base, a phi carries
            # it because it is live around the loop, and nothing writes it
            # again. "The register is written twice" refuses hotlop's load
            # of `n`, also safe: the loop writes ax on every line and the
            # load is consumed where it stands. What is unsafe is the pair --
            # a value a phi carries whose register the loop goes on to
            # change, which is segld's inner counter and 1030 for 1050.
            if any(
                value in begins
                and origin.get(value) in twice
                and (readable is None or value in readable)
                for value in one.defines
                if not value.flags
            ):
                continue
            # An operand nothing writes down used to end the run here.
            # hotlop hoisted `mov ax,[n] / imul word [k]`, the recolour
            # renamed the load to cx, and the multiply went on reading ax:
            # 0 for 630. Two things had to exist before that could be
            # allowed. regalloc.required() now refuses an assignment that
            # puts such an operand anywhere but where its instruction reads
            # it, and a result the machine places is copied out rather than
            # re-seated -- see `copied` below.
            if any(
                ref.addr is None or mir.overlapping(ref, other, dgroup, bounds)
                for ref in one.loads
                for other in stores
            ):
                continue
            # A phi result is the loop-carried value itself: `v2` at
            # hotlop's header is the counter. Collecting only op.defines
            # left every phi looking like something defined outside, so a
            # compare against the counter read as invariant.
            inside = {value for other in ops for value in other.defines} | carried
            # Per use, not per value: hotlop's counter is genuinely read by
            # the compare, and that must not make the load of `n` -- which
            # only preserves the register it lives in -- loop-carried too.
            read = _reads(one)
            blocking = [
                use
                for use in one.uses
                if not use.flags and ir.ROOT.get(origin.get(use, -1), origin.get(use, -1)) in read
            ]
            if any(use in inside and use not in made for use in blocking):
                continue
            run.append(one)
            made.update(one.defines)
            changing = True

    thinning = True
    while thinning:
        thinning = False
        for one in run:
            if one.kind is not mir.Kind.COPY or one.loads:
                continue
            if any(value in other.uses for other in run if other is not one for value in one.defines):
                continue
            run = _pruned(run, set(one.defines))
            thinning = True
            break
    return run


def _pruned(run: list, drop: set) -> list:
    """The run without the values named, and without whatever fed only them.

    Dropping the operation that defines a value leaves anything reading it
    computing from something no longer there, so this runs to a fixed point
    rather than filtering once.
    """
    keep = [one for one in run if not (set(one.defines) & drop)]
    changing = True
    while changing:
        changing = False
        gone = {value for one in run if one not in keep for value in one.defines}
        for one in list(keep):
            if any(use in gone for use in one.uses):
                keep.remove(one)
                changing = True
    return keep


def _crossing(run: list, rest: list, phis: list | None = None, wanted: set | None = None) -> frozenset | None:
    """The values the run computes that the rest of the loop still reads.

    Each needs a register of its own, so this used to insist on exactly one.
    The count is not the constraint, the registers are, and the caller
    finds that out by asking the allocator. And no flag among them:
    a flag cannot be carried to the loop in a register, so a comparison
    whose answer is read inside it has to stay inside it. Counting only
    non-flag values here is what let hotlop's compare move out from under
    the branch that reads it.
    """
    # A phi carries a value out of the run as surely as an instruction
    # reads one, and `rest` holds no phis: hotlop's high half reached the
    # next iteration that way, seen by nothing here.
    taken = {value for other in rest for value in other.uses} | {
        value for phi in (phis or ()) for value in phi.incoming.values()
    }
    if wanted is not None:
        taken &= wanted
    crossing = {value for one in run for value in one.defines if value in taken}
    if not crossing or any(value.flags for value in crossing):
        return None
    return frozenset(crossing)


def _span_of(op: Op) -> tuple[int, int] | None:
    """The bytes this operation occupied before anything moved it."""
    return ir.span(op.node) if op.node is not None else None


def _named(register: Register_, width: int) -> Register_:
    """The same register named at the width an operand needs.

    _at_width returns None where there is no such name -- there is no
    byte-wide si -- and every caller here has already picked a register
    that has one, so the fallback is the register itself rather than a
    refusal that cannot happen.
    """
    return _at_width(register, width) or register


def _mentions(one: ir.Loc, register: Register_) -> bool:
    """Whether this operand names the register, held in or reached through."""
    root = ir.ROOT.get(register, register)
    if isinstance(one, ir.Reg):
        return ir.ROOT.get(one.register, one.register) is root
    for where in (
        getattr(one, "through", None),
        getattr(one, "index", None),
        getattr(getattr(one, "addr", None), "base", None),
    ):
        if where is not None and ir.ROOT.get(where, where) is root:
            return True
    return False


def _instead(one: ir.Loc, was: Register_, now: Register_) -> ir.Loc:
    """This operand with one register put in place of another."""
    root = ir.ROOT.get(was, was)
    if isinstance(one, ir.Reg) and ir.ROOT.get(one.register, one.register) is root:
        return replace(one, register=_named(now, one.width))
    if not isinstance(one, (ir.Mem, ir.Address)):
        return one
    swap = {}
    for name in ("through", "index"):
        where = getattr(one, name, None)
        if where is not None and ir.ROOT.get(where, where) is root:
            swap[name] = _named(now, 2)
    addr = getattr(one, "addr", None)
    if addr is not None and ir.ROOT.get(addr.base, addr.base) is root:
        swap["addr"] = replace(addr, base=_named(now, 2))
    return replace(one, **swap) if swap else one


def _reads_from(one: Op, was: Register_, now: Register_) -> Op | None:
    """The operation reading `now` where it read `was`, or None if it cannot.

    It cannot when the same register is also written -- `add ax,[s]` with ax
    holding the hoisted value would put the sum in the register that value
    lives in, and the next pass of the loop would read the sum. Nor when an
    operand is implicit, since there is nothing written down to change.
    Those readers take a copy instead.
    """
    what = _semantics_of(one)
    if what is None or _implicit(one):
        return None
    root = ir.ROOT.get(was, was)
    if any(_mentions(where, root) for where in what.dests):
        return None
    return replace(one, made=replace(what, sources=tuple(_instead(where, root, now) for where in what.sources)))


def _move(at: int, into: Register_, outof: Register_, value: mir.Value) -> Op:
    """`mov into,outof`, both registers spelled out.

    `covers` is empty: it stands for none of BC's bytes, which is what keeps
    the coverage arithmetic adding up and what stops a later round hoisting
    it in turn.
    """
    what = ir.Semantics(
        ir.Operation.MOVE,
        "mov",
        dests=(ir.Reg(register=_named(into, 2), width=2),),
        sources=(ir.Reg(register=_named(outof, 2), width=2),),
    )
    return Op(at, ir.Operation.MOVE, "mov", (), (value,), kind=mir.Kind.COPY, made=what, covers=(at, at))


def _can_reseat(one: Op) -> bool:
    """Whether _writes_to can actually move this operation's destination.

    Not a two-address one. `add bx,ax` reads bx and writes it, and x86 says
    so by naming the operand once -- so rewriting the destination rewrites
    the source with it. _writes_to touches only `dests` and produces
    `add cx,ax`, which accumulates into a register the chain never put
    anything in: pressx sums four invariant products into bx and printed
    R= 6580 for 7500. lir.tied is where that is written down.
    """
    what = _semantics_of(one)
    if what is None or len(what.dests) != 1 or not isinstance(what.dests[0], ir.Reg):
        return False
    return lir.tied(what) is None


def _writes_to(one: Op, now: Register_) -> Op:
    """The operation with its single destination register put somewhere else."""
    what = _semantics_of(one)
    if what is None or len(what.dests) != 1 or not isinstance(what.dests[0], ir.Reg):
        return one
    width = what.dests[0].width
    return replace(one, made=replace(what, dests=(ir.Reg(register=_named(now, width), width=width),)))


# Operations the body can be observed through, whatever they define. A
# store leaves a mark, a call and the control transfers take the program
# somewhere, and everything touching the stack moves sp. The x87 forms are
# here because their effect is on a stack this layer does not model.
#
# JOIN is deliberately not here. The list used to name ir.Operation.RESTORE
# and that entry never once matched: the raise gives a restore its own
# Synth op, so the name it was compared against was never the name it had.
# Adding it for real cost 1,134 bytes over the corpus and 20 deletions --
# a join whose result nothing reads is as dead as anything else.
_OBSERVED = frozenset(
    {
        mir.Kind.CALL,
        mir.Kind.RETURN,
        mir.Kind.JUMP,
        mir.Kind.BRANCH,
        mir.Kind.ESCAPE,
        mir.Kind.ARG,
        mir.Kind.RESULT,
        mir.Kind.OPAQUE,
        mir.Kind.FLOAD,
        mir.Kind.FSTORE,
        mir.Kind.FADD,
        mir.Kind.FSUB,
        mir.Kind.FMUL,
        mir.Kind.FDIV,
        mir.Kind.FNEG,
        mir.Kind.FCOMPARE,
    }
)


def _leaving(body: MirBody) -> set:
    """The value each register holds where control leaves the body.

    What the caller reads is not a fact this body holds, so everything that
    reaches an exit counts as read. Reaching definitions forward, meeting by
    union: two definitions of one register arriving at a join are both still
    readable there, and claiming otherwise would kill a live one.

    This is the one place the liveness looks at `origin`. It has to: "what
    the caller sees" is a statement about registers, and there is nothing
    else in a MirBody that says which value ends up where.
    """
    preds = {block.at: [one.at for one in body.blocks if block.at in one.succ] for block in body.blocks}
    arriving: dict = {}
    for value in regalloc.entry_values(body):
        register = body.origin.get(value)
        if register is not None:
            arriving.setdefault(ir.ROOT.get(register, register), set()).add(value)

    outof: dict[int, dict] = {block.at: {} for block in body.blocks}
    changing = True
    while changing:
        changing = False
        for block in body.blocks:
            here: dict = {}
            coming = [outof[one] for one in preds[block.at]]
            if block.at == body.entry:
                coming.append(arriving)
            for one in coming:
                for register, values in one.items():
                    here.setdefault(register, set()).update(values)
            for phi in block.phis:
                register = body.origin.get(phi.result)
                if register is not None:
                    here[ir.ROOT.get(register, register)] = {phi.result}
            for op in block.ops:
                for value in op.defines:
                    if value.flags:
                        continue
                    register = body.origin.get(value)
                    if register is not None:
                        here[ir.ROOT.get(register, register)] = {value}
            if here != outof[block.at]:
                outof[block.at] = here
                changing = True

    out: set = set()
    for block in body.blocks:
        if block.succ:
            continue
        for values in outof[block.at].values():
            out |= values
    return out


# A value is a 32-bit register and this machine's code is 16-bit, so the
# two halves of one are read and written independently. LOW is what a
# narrow operand names; HIGH is what a narrow write leaves alone.
LOW, HIGH = 0, 1


def halves(body: MirBody) -> set:
    """Which half of which value something reads, to a fixed point.

    Whole-value liveness cannot answer the question this machine asks. Every
    narrow write is a read-modify-write here -- `mov bx,2EEh` writes bx and
    preserves the top half of ebx, so it reads the ebx before it -- and
    counting that as reading the value keeps the previous write alive for
    ever. press's `mov bx,cx`, overwritten two bytes later, was live on the
    strength of a half nothing wanted.

    So the unit is (value, half). A narrow write takes HIGH from the value
    it overwrites and nothing else, which is what makes a chain of them
    collapse: each link is read only for a half, and where no one ever reads
    that half the whole chain is dead.

    Both halves of everything reaching an exit are live, because what the
    caller reads is not a fact this body holds.
    """
    out: set = set()
    for value in _leaving(body):
        out.add((value, LOW))
        out.add((value, HIGH))

    def widths(op: Op) -> dict:
        """The widest each value is read at by this operation.

        By value, not by register root through `origin`. Asked the old way,
        an operand a pass had rewritten to name a value was not an ir.Reg
        and so matched no root -- and a folded `mov eax,186A0h` whose only
        reader was such an operand looked unread. lngmix printed 110 for
        142900: the dividend was deleted and idiv divided whatever was in
        eax.
        """
        found: dict = {}
        for one in op.args:
            if isinstance(one, mir.Held):
                found[one.value] = max(found.get(one.value, 0), one.width)
        return found

    changing = True
    while changing:
        before = len(out)
        for block in body.blocks:
            for op in block.ops:
                if (
                    op.kind not in _OBSERVED
                    and not op.stores
                    and not any((one, half) in out for one in op.defines for half in (LOW, HIGH))
                ):
                    continue
                carried = _carried(op, body.origin)
                read = widths(op)
                described = op.kind is not mir.Kind.OPAQUE and not op.barrier
                for one in op.uses:
                    if one in carried:
                        # Read for the half it is merged into, and only if
                        # something reads that half of the result.
                        if (carried[one], HIGH) in out:
                            out.add((one, HIGH))
                        continue
                    if not described or one not in read:
                        # Nothing written down says how much of it is read.
                        out.add((one, LOW))
                        out.add((one, HIGH))
                        continue
                    out.add((one, LOW))
                    if read[one] >= 4:
                        out.add((one, HIGH))
                for ref in op.loads + op.stores:
                    for one in (ref.base, ref.segment):
                        if one is not None:
                            out.add((one, LOW))
                            out.add((one, HIGH))
            for phi in block.phis:
                for half in (LOW, HIGH):
                    if (phi.result, half) in out:
                        out |= {(one, half) for one in phi.incoming.values()}
        changing = len(out) != before
    return out


def live(body: MirBody) -> set:
    """Values some half of which something reads.

    halves() is the analysis; this is the projection every caller that only
    asks "is this value read at all" wants.
    """
    return {one for one, _ in halves(body)}


def _carried(op: Op, origin: dict) -> dict:
    """Uses that are only the previous contents of a register being written.

    A 16-bit write under a 32-bit register model is a read-modify-write, so
    raising `imul word [n]` -- which reads ax and memory and writes dx:ax --
    gives the operation a use of dx as well, standing for the half of edx it
    leaves alone. That read is real, and worth exactly what the write it
    feeds is worth: hotlop's dx result is dead, so the dx it merges into is
    dead too. Treating every use of a live operation as live keeps a whole
    loop-invariant multiply alive on the strength of it.

    A use is carried when it shares a register with something the operation
    defines and the operation's own semantics never name that register as a
    source.
    """
    semantics = _semantics_of(op)
    if semantics is None:
        return {}
    # Semantics naming no operand describe nothing, so nothing here is a
    # partial write. A call names none, and every argument it reads shares
    # a register with something it clobbers -- so all of them would read as
    # the register's previous value and none as an input. B$OGTA takes the
    # ON GOTO branch index in bx and the `mov bx` before it read as dead.
    if not semantics.sources and not semantics.dests:
        return {}

    def root(one):
        register = origin.get(one)
        return None if register is None else ir.ROOT.get(register, register)

    named = {
        ir.ROOT.get(one.register, one.register) for one in semantics.sources if isinstance(one, ir.Reg)
    }
    out: dict = {}
    for value in op.defines:
        if value.flags:
            continue
        where = root(value)
        if where is None or where in named:
            continue
        for one in op.uses:
            if root(one) == where:
                out[one] = value
    return out


# What each conditional jump asks of `cmp a,b`, as a predicate on the two
# operands. Signed and unsigned are different questions and BC emits both.
# Whether a comparison holds. Keyed on what the branch tests, which the
# raise decided; keyed on the branch's mnemonic this was a pass that had to
# know x86 spells "less than" six different ways.
_TAKEN = {
    mir.Kind.EQ: lambda a, b, u: a == b,
    mir.Kind.NE: lambda a, b, u: a != b,
    mir.Kind.LT: lambda a, b, u: a < b,
    mir.Kind.LE: lambda a, b, u: a <= b,
    mir.Kind.GT: lambda a, b, u: a > b,
    mir.Kind.GE: lambda a, b, u: a >= b,
    mir.Kind.BELOW: lambda a, b, u: u(a) < u(b),
    mir.Kind.BELOW_EQ: lambda a, b, u: u(a) <= u(b),
    mir.Kind.ABOVE: lambda a, b, u: u(a) > u(b),
    mir.Kind.ABOVE_EQ: lambda a, b, u: u(a) >= u(b),
}


def _signed(fact) -> int:
    """A Known as the number the machine would compare."""
    top = 1 << (fact.width * 8 - 1)
    return fact.n - (top << 1) if fact.n & top else fact.n


def _outcome(block, op: Op, facts: dict, held: dict, origin: dict) -> bool | None:
    """Whether this branch is taken, where both its operands are numbers."""
    if op.kind is not mir.Kind.BRANCH:
        return None
    decide = _TAKEN.get(op.test)
    if decide is None:
        return None
    reads = [one for one in op.uses if one.flags]
    if len(reads) != 1:
        return None

    # The comparison this branch reads, which must be the last thing to
    # write the flags before it -- SSA says so by naming the value.
    where = next(
        (
            (index, one)
            for index, one in enumerate(block.ops)
            if reads[0] in one.defines
        ),
        None,
    )
    if where is None:
        return None
    index, compare = where
    # A comparison in MIR's own terms: it subtracts and keeps only the
    # flags, so its two operands are its args. Asking the instruction meant
    # matching ir.Operation.COMPARE and reading ir.Loc operands out of it.
    if compare.kind is not mir.Kind.SUB or len(compare.args) != 2 or compare.results:
        return None
    parts = [
        consts._operand(compare, one, facts, held.get((block.at, index)))
        for one in compare.args
    ]
    if any(one is None for one in parts):
        return None
    left, right = parts
    return decide(_signed(left), _signed(right), lambda n: n & 0xFFFFFFFF)


def decided(body: MirBody, dgroup: frozenset[int], calls: dict[int, str]) -> MirBody:
    """A branch on two numbers, resolved.

    `IF a < b` with both constants is decided where it stands, and leaving
    it to run costs the comparison, the jump, and everything on the arm
    that cannot be taken. bools is three of them over four constants and
    nothing else.

    Taken becomes an unconditional jump and not-taken goes entirely, its
    bytes handed to the operation before it. What becomes unreachable is
    dropped by resolving the body afterwards rather than here.
    """
    facts = consts.known(body, dgroup, calls)
    if not facts:
        return body
    held = consts.cells(body, dgroup, calls, facts)

    out = []
    changed = False
    for block in body.blocks:
        if not block.ops:
            out.append(block)
            continue
        last = block.ops[-1]
        answer = _outcome(block, last, facts, held, body.origin)
        if answer is None:
            out.append(block)
            continue
        target = last.target
        if target is None or target not in {one.at for one in body.blocks}:
            out.append(block)
            continue
        changed = True
        if answer:
            jump = replace(
                last,
                op=ir.Operation.JUMP,
                kind=mir.Kind.JUMP,
                name="jmp",
                uses=(),
                args=(),
                results=(),
                target=target,
                made=None,
                covers=last.covers or _span_of(last),
            )
            out.append(replace(block, ops=block.ops[:-1] + (jump,)))
        else:
            kept = _absorb(list(block.ops), {last.at})
            if len(kept) == len(block.ops):
                out.append(block)
                continue
            out.append(replace(block, ops=tuple(kept)))
    if not changed:
        return body
    # The successors are left as they were, and the body is not resolved.
    # Both would be tidier and both lose bytes: resolving prunes a block
    # nothing can reach any more, and layout.py then has a hole it cannot
    # account for -- `0x006d: 3 bytes between the ops are not instructions`
    # on three of the bools objects. An edge that can no longer be taken is
    # an over-approximation of the control flow, which is the safe
    # direction for everything that reads it.
    return replace(body, blocks=tuple(out))


def dead(body: MirBody) -> MirBody:
    """Operations whose results nothing reads, removed.

    BC emits no dead code -- 7 operations in the whole corpus before any
    pass runs -- so this is for what the passes leave behind: folding turns
    a load into a move of a number and what fed it stops being read, and
    hoisting takes a computation out and leaves the copy standing in for it.

    live() is the analysis, and what it needs is that every use list is
    complete. For a call that means its contract naming the registers it
    reads, which is runtime.Contract.inputs and was written for this.
    Removing an operation hands its bytes to the one before it -- layout.py
    requires that and _absorb() already does it, refusing where no survivor
    is adjacent to take them.
    """
    # Not in a body holding something this cannot read. A use list is only
    # as complete as the semantics behind it, and an opaque or emulated
    # instruction reads registers none of them mention: byref2 printed 0
    # for 16 that way.
    if any(
        one.barrier or one.kind is mir.Kind.OPAQUE
        for block in body.blocks
        for one in block.ops
    ):
        return body

    alive = live(body)
    out = []
    changed = False
    for block in body.blocks:
        # Absorption puts several operations on one address and _absorb
        # keys on it, so an address shared with something live is not one
        # this may name.
        seen: dict[int, int] = {}
        for op in block.ops:
            seen[op.at] = seen.get(op.at, 0) + 1
        gone = {op.at for op in block.ops if seen[op.at] == 1 and _removable(op, alive)}
        if not gone:
            out.append(block)
            continue
        ops = _absorb(list(block.ops), gone)
        changed = changed or len(ops) != len(block.ops)
        out.append(replace(block, ops=tuple(ops)))
    return replace(body, blocks=tuple(out)) if changed else body


def _removable(op: Op, alive: set) -> bool:
    """Whether anything at all would notice this operation going."""
    if op.kind in _OBSERVED or op.stores or op.barrier:
        return False
    if op.kind is mir.Kind.OPAQUE:
        return False
    if not [one for one in op.defines if not one.flags]:
        return False
    return not any(one in alive for one in op.defines)


def folded(body: MirBody, dgroup: frozenset[int], calls: dict[int, str]) -> MirBody:
    """An operation whose result is a number, replaced by that number.

    `n = 7 : k = 3` and then `n * k` inside a loop is three stores and two
    loads and a multiply, all of it computing 21 on every pass. consts.known
    says which values are numbers -- reading through memory, because BC
    keeps every variable there and nothing else would reach past the first
    store -- and this writes them down.

    SSA in, SSA out: the operation goes on defining the value it defined,
    and simply computes it from nothing. What it read it no longer reads,
    so its uses and loads go, which is what lets dead code elimination see
    the loads afterwards.

    Refused where the flags it also sets are read. `add ax,[c]` computes a
    number and a carry, and only one of them is expressible as `mov ax,n`.
    """
    facts = consts.known(body, dgroup, calls)
    if not facts:
        return body

    # Live, not merely mentioned: see live()'s own note on hotlop's dx.
    wanted = live(body)

    out = []
    changed = False
    for block in body.blocks:
        ops = []
        for op in block.ops:
            made = _folded_op(op, facts, wanted, body.origin)
            changed = changed or made is not op
            ops.append(made)
        out.append(replace(block, ops=tuple(ops)))
    return replace(body, blocks=tuple(out)) if changed else body


def _folded_op(op: Op, facts: dict, wanted: set, origin: dict) -> Op:
    """The operation as a move of its own answer, where that is possible."""
    what = _semantics_of(op)
    if what is None or op.stores:
        return op
    # Not a register move. Rewriting `mov ax,cx` to `mov ax,3` removes no
    # work -- the same instruction, the same length, nothing read that was
    # not already in a register -- and it undoes an allocation: a live range
    # split is exactly that move, so folding it puts the computation back
    # inside the loop the hoist took it out of, and the hoist lifts it again
    # next round. Three bytes a round, for ever.
    #
    # What folding is for is removing a computation. A copy is not one.
    if op.kind is mir.Kind.COPY:
        return op
    if op.kind in (mir.Kind.JUMP, mir.Kind.BRANCH, mir.Kind.CALL, mir.Kind.RETURN):
        return op
    if not what.dests or not isinstance(what.dests[0], ir.Reg):
        return op

    # Which value the first destination names. A widening `imul` has two --
    # dx:ax -- and its answer is the low half; hotlop's `n * k` is that
    # operation, both halves constant and the high one dead.
    target = consts._defined(op, what, origin)
    if target is None or target not in facts:
        return op
    # A second result that something reads is not expressible as one move.
    if any(one != target and one in wanted for one in op.defines):
        return op

    fact = facts[target]
    into = what.dests[0]
    if op.kind is mir.Kind.COPY and any(isinstance(one, mir.Const) for one in op.args):
        return op  # already says so

    # In MIR's own operands: this value, that constant. What register it
    # ends up in is the allocator's, and lower.py is where it becomes one.
    return replace(
        op,
        op=ir.Operation.MOVE,
        kind=mir.Kind.COPY,
        name="mov",
        defines=(target,),
        uses=(),
        loads=(),
        args=(mir.Const(fact.n, into.width),),
        results=(mir.Held(target, into.width),),
        made=None,
    )


def _insertion(
    body: MirBody,
    alive,
    block,
    run: list,
    crossing,
    touched: set,
    reached_by: set,
    addressable: set,
    readable: set,
) -> tuple[int, list] | None:
    """Where in the preheader a run can go, and what registers are free there.

    Appending is one choice out of many and often the wrong one. A run needs
    two things that pull in opposite directions: every value it reads has to
    be defined *before* it, and every register it writes has to be free
    *at* it. Late satisfies the first, early the second, and for the loops
    that matter neither end satisfies both.

    hotlpx is the shape. Its preheader reads two variables through runtime
    calls and then starts the counter in ax, and the run needs ax for the
    multiplicand. After the counter, ax is taken; before the calls, nothing
    survives them. The only place it goes is between: after the last call,
    ahead of the counter, where ax is not yet in use and bx is no longer in
    the way.

    So this walks positions from the end back to the earliest the run's own
    operands allow, and returns the first that works -- latest, because that
    is the shortest the result has to stay live.
    """
    ops = list(block.ops)
    entering = set(alive.live_in.get(block.at, ()))
    leaving = set(alive.live_out.get(block.at, ()))
    defined = [{value for value in one.defines if not value.flags} for one in ops]

    def root(value):
        register = body.origin.get(value)
        return None if register is None else ir.ROOT.get(register, register)

    def roots(values):
        return {root(one) for one in values} - {None}

    # The earliest position the run's own operands permit: after the last
    # thing in this block that it reads.
    wanted = {value for one in run for value in one.uses}
    earliest = 0
    for index, made in enumerate(defined):
        if made & wanted:
            earliest = index + 1

    # The run keeps the registers its operations were written with -- a
    # result the machine places is copied out, not re-seated -- so those
    # have to be free wherever it lands.
    keeps = roots({value for one in run for value in one.defines if not value.flags})

    for index in range(len(ops), earliest - 1, -1):
        before = set(entering)
        for made in defined[:index]:
            before |= made
        # Not a use that is only the register's previous value. Every
        # 16-bit write is a read-modify-write here, so `mov ax,1` reads the
        # eax before it -- and counting that as a read means no register is
        # ever free between two writes to it, which is to say nowhere is a
        # legal place to put anything. BC's code is 8086 and never writes a
        # high half, so there is nothing in the half being preserved.
        after = {
            value
            for one in ops[index:]
            for value in one.uses
            if value not in _carried(one, body.origin)
        }
        # Live in the sense that something reads it. A join raises a phi
        # for every register, so a dead call result in dx looks live out of
        # this block and makes dx busy everywhere after the call -- which is
        # exactly where the run needs to go. See live() and _carried.
        across = before & (after | leaving) & readable
        busy = roots(across)
        if keeps & busy:
            continue
        # The result has to survive from here to the loop, so it may not sit
        # anywhere this block writes later on.
        later = roots({value for made in defined[index:] for value in made})
        spare = [
            where
            for where in regalloc.AVAILABLE
            if where not in busy and where not in later and where not in touched and where not in keeps
        ]
        if len(spare) < len(crossing):
            continue
        if any(
            value in reached_by and not (set(spare) & addressable)
            for value in crossing
        ):
            continue
        return index, spare
    return None


def hoisted(body: MirBody, dgroup: frozenset[int], calls: dict[int, str], bounds: dict | None = None) -> MirBody:
    """A loop-invariant run of operations, done once before the loop.

    `mov ax,[n] / imul word [k]` computes the same product on every pass of
    a loop that writes neither. Moving the whole run rather than the load
    alone is what makes it possible at all: `imul`'s multiplicand is ax
    implicitly, so renaming the load's destination leaves the multiply
    reading a register nothing put anything in -- hotlop printed 0 for 630
    that way. With the run moved, the implicit registers are used inside it,
    in the preheader, and only its result has to survive into the loop.

    That result needs a register the loop does not touch, which is what
    `pins` asks regalloc for. Refused, the whole thing is dropped: a value
    hoisted into a register the loop clobbers is the wrong program.
    """
    inside = loopy.loops(list(body.blocks), body.entry)
    if not inside:
        return body
    at_of = {block.at: block for block in body.blocks}
    alive = regalloc.live(body)
    readable = live(body)
    effective = _effective(body, calls)
    reached_by = regalloc._addressing(body)
    addressable = {ir.ROOT.get(one, one) for one in regalloc.ADDRESSING}
    moved: dict[int, list[Op]] = {}
    gone: set[int] = set()
    seated: dict[int, Register_] = {}
    copied: dict[int, list] = {}
    placing: dict[int, int] = {}
    claimed: set = set()
    rewrites: dict[int, Op] = {}
    splits: dict[int, list] = {}

    for loop in inside:
        into = _preheader(body, loop)
        if into is None or into in loop.body:
            continue
        ops = [one for at in sorted(loop.body) for one in at_of[at].ops]
        stores = [ref for one in ops for ref in one.stores]
        carried = {phi.result for at in loop.body for phi in at_of[at].phis}
        phis = [phi for at in loop.body for phi in at_of[at].phis]
        run = _invariant_run(ops, carried, stores, dgroup, calls, body.origin, bounds, _starts(phis), readable)
        # Not one already taken out of a loop inside this one. Invariant in
        # the inner loop and in the outer, it was put in both preheaders and
        # its bytes counted twice, which layout reports as a negative gap:
        # harr and segld nest ten by ten.
        run = [one for one in run if one.at not in gone]
        if not run:
            continue

        # What the run computes that the rest of the loop still reads. One
        # value, or this would need a register for each and a rule for
        # which; the shapes that pay have exactly one.
        rest = [one for one in ops if one not in run]
        crossing = _crossing(run, rest, phis, effective)
        if crossing is None:
            continue

        touched = set()
        for one in ops:
            what = _semantics_of(one)
            for where in ((*what.dests, *what.sources) if what else ()):
                if isinstance(where, ir.Reg):
                    touched.add(ir.ROOT.get(where.register, where.register))
        touched |= {
            ir.ROOT.get(body.origin.get(value, -1), -1)
            for at in loop.body
            for value in alive.live_out.get(at, ())
            if not value.flags
        }

        # Where it goes, and what is free there. Both answers at once,
        # because neither can be had without the other: see _insertion.
        found = _insertion(
            body, alive, at_of[into], run, crossing, touched, reached_by, addressable, readable
        )
        if found is None:
            continue
        index, spare = found

        # One register per value, and the whole rewrite done here rather
        # than asked of the allocator. Moving a definition's register means
        # rewriting every consumer that reads it, and those are one
        # transformation: doing the first alone emits `mov di,0` with the
        # loop still reading `[si+0Ah]`, and doing neither leaves the moved
        # operation writing over whatever the preheader had there.
        here: dict = {}
        taken = set(seated.values()) | claimed
        for result in sorted(crossing, key=lambda one: one.id):
            free = [
                where
                for where in spare
                if where not in taken
                and (result not in reached_by or ir.ROOT.get(where, where) in addressable)
            ]
            if not free:
                break
            here[result] = free[0]
            taken.add(free[0])
        if len(here) != len(crossing):
            continue

        # Every reader, and how it is served. One that only reads the value
        # has the register swapped in its operands. One that also writes it,
        # or whose operand is implicit -- `imul word [b]` multiplies by ax
        # and names it nowhere -- gets the value put back just before it
        # runs, which is what a live range split is.
        served: dict[int, Op] = {}
        copies: dict[int, list] = {}
        beaten = False
        for other in rest:
            for result in crossing:
                if result not in other.uses:
                    continue
                was = body.origin.get(result)
                if was is None:
                    beaten = True
                    break
                again = _reads_from(served.get(other.at, other), was, here[result])
                if again is not None:
                    served[other.at] = again
                else:
                    copies.setdefault(other.at, []).append((was, here[result]))
            if beaten:
                break
        if beaten:
            continue

        # Re-seating an operation means rewriting its destination, and not
        # every operation can have one written. A widening `imul` writes
        # dx:ax and names neither, so _writes_to leaves it alone -- and the
        # readers, already rewritten to read the new register, then read one
        # nothing wrote. That is the 0-for-630 this used to refuse rather
        # than risk.
        #
        # So where the machine says where the result lands, the result is
        # left there and copied out. The copy costs a move in the preheader,
        # which runs once, against work that ran every pass of the loop.
        for one in run:
            what = _semantics_of(one)
            fixed = lir.writes(what) if what is not None else {}
            for value in one.defines:
                if value not in here:
                    continue
                was = ir.ROOT.get(body.origin.get(value, -1), -1)
                # Copy out where the operation cannot be told where to
                # write: the machine placed the result, or -- as with
                # calls.py's restore idiom, whose semantics name no operand
                # at all -- there is no destination written down to change.
                # _writes_to returns such an operation untouched and says
                # nothing, while the readers have already been pointed at
                # the new register: lngmix emitted `mov [bp-14h],di` with
                # nothing anywhere writing di.
                placed = fixed.get(was) is not None and fixed[was].fixed is not None
                if placed or not _can_reseat(one):
                    copied.setdefault(one.at, []).append((was, here[value], value))
                    claimed.add(here[value])
                else:
                    seated[one.at] = here[value]
        rewrites.update(served)
        splits.update(copies)
        placing[into] = min(placing.get(into, index), index)
        moved[into] = moved.get(into, []) + list(run)
        gone.update(one.at for one in run)

    if not gone:
        return body

    out = []
    for block in body.blocks:
        was = [one.at for one in block.ops if one.at not in gone]
        ops = [rewrites.get(one.at, one) for one in block.ops if one.at not in gone]

        # A copy ahead of every reader served by one, keyed on the address
        # that reader had before anything was re-stamped onto it.
        ahead: list[Op] = []
        for one, before in zip(ops, was, strict=True):
            for old, now in splits.get(before, ()):
                ahead.append(_move(one.at, old, now, mir.Value(-1, one.at)))
            ahead.append(one)
        ops = ahead

        if ops and block.ops and block.ops[0].at in gone:
            # Onto the block's own address so branches still land, and told
            # what it stands for: `covers` otherwise means the bytes at
            # `at`, which are now the hoisted operation's, and both would
            # claim them.
            first = ops[0]
            ops = [replace(first, at=block.ops[0].at, covers=first.covers or _span_of(first))] + ops[1:]
        if block.at in moved:
            leaves = bool(ops) and ops[-1].kind in (mir.Kind.JUMP, mir.Kind.BRANCH)
            lifted = []
            for one in moved[block.at]:
                if one.at in copied:
                    lifted.append(one)
                    lifted += [_move(one.at, now, was, value) for was, now, value in copied[one.at]]
                elif one.at in seated:
                    lifted.append(_writes_to(one, seated[one.at]))
                else:
                    lifted.append(one)
            # Running in the preheader and still standing for the bytes it
            # came from, which is what a record or fixup naming those bytes
            # needs in order to follow it.
            # The address is what orders these once the body is emitted, so
            # a position in the list is not enough -- they have to take the
            # address of whatever they go in front of. `covers` is what says
            # which of BC's bytes they stand for, and is untouched.
            index = placing.get(block.at, len(ops))
            if leaves and index >= len(ops):
                index = max(0, len(ops) - 1)
            if ops:
                anchor = ops[index].at if index < len(ops) else ops[-1].at
                lifted = [
                    replace(one, at=anchor, covers=one.covers or _span_of(one)) for one in lifted
                ]
            ops = ops[:index] + lifted + ops[index:]
        out.append(replace(block, ops=tuple(ops)))

    # In SSA again, because the operations have moved and the phis raised
    # with the original body no longer describe them: a load hoisted out of
    # a loop was loop-carried and is now live once, ahead of it. Nothing
    # after this asks the allocator anything -- every register is written
    # down -- so a refusal here is the only way the rewrite can fail.
    laid = mir.resolved(replace(body, blocks=tuple(out)), calls)
    return body if isinstance(laid, str) else laid


def _renamed(was: MirBody, now: MirBody, wanted: set) -> dict | None:
    """Each value of interest, as it is named after the body was resolved.

    By where it is defined and which register it lands in, not by position
    in the map: `resolved` may give an op more defines than it had, since it
    unions the node's own effects with the semantics a transform chose. The
    op is the same op at the same index, and within it a register is defined
    once.
    """
    place = {}
    for block in was.blocks:
        for index, op in enumerate(block.ops):
            for value in op.defines:
                if value in wanted:
                    place[value] = (block.at, index, was.origin.get(value))

    at_of = {block.at: block for block in now.blocks}
    out = {}
    for value, (at, index, register) in place.items():
        block = at_of.get(at)
        if block is None or index >= len(block.ops):
            return None
        here = [one for one in block.ops[index].defines if now.origin.get(one) is register]
        if len(here) != 1:
            return None
        out[value] = here[0]
    return out if len(out) == len(wanted) else None


def _choices(offers: dict) -> Iterator[dict]:
    """Each way of giving every pinned value one of its candidate registers."""
    values = list(offers)
    for picked in product(*(offers[one] for one in values)):
        if len(set(picked)) == len(picked):
            yield dict(zip(values, picked, strict=True))


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



# Every transform, as the one thing a transform is. The functions above stay
# because they are what each class does and are what the tests name; what
# changes is that the pipeline can only reach them through `transform`.
class Fold(MIRTransform):
    name = "fold"

    def __init__(self, where: Where) -> None:
        self.where = where

    def transform(self, body: MirBody) -> MirBody:
        return folded(body, self.where.dgroup, self.where.named)


class Decide(MIRTransform):
    name = "decide"

    def __init__(self, where: Where) -> None:
        self.where = where

    def transform(self, body: MirBody) -> MirBody:
        return decided(body, self.where.dgroup, self.where.named)


class Dead(MIRTransform):
    name = "dead"

    def transform(self, body: MirBody) -> MirBody:
        return dead(body)


class Segments(MIRTransform):
    name = "segments"

    def __init__(self, where: Where) -> None:
        self.where = where

    def transform(self, body: MirBody) -> MirBody:
        return segments(body, self.where.dgroup, self.where.named)


class Hoist(MIRTransform):
    name = "hoist"

    def __init__(self, where: Where) -> None:
        self.where = where

    def transform(self, body: MirBody) -> MirBody:
        return hoisted(body, self.where.dgroup, self.where.named, self.where.bounds)


class Forward(MIRTransform):
    name = "forward"

    def __init__(self, where: Where) -> None:
        self.where = where

    def transform(self, body: MirBody) -> MirBody:
        return forwarded(body, self.where.dgroup, self.where.named)


class DropLoads(MIRTransform):
    name = "drop_loads"

    def __init__(self, where: Where) -> None:
        self.where = where

    def transform(self, body: MirBody) -> MirBody:
        return without_redundant_loads(body, self.where.dgroup, self.where.named)


class DropStores(MIRTransform):
    name = "drop_stores"

    def __init__(self, where: Where) -> None:
        self.where = where

    def transform(self, body: MirBody) -> MirBody:
        return without_dead_stores(body, self.where.dgroup, self.where.named)


class Widen(MIRTransform):
    name = "widen"

    def transform(self, body: MirBody) -> MirBody:
        return widened(body)


class Place(MIRTransform):
    name = "place"

    def transform(self, body: MirBody) -> MirBody:
        return placed(body)


def pipeline(where: Where, **wanted) -> list[MIRTransform]:
    """The passes, in order, that `wanted` leaves on.

    Order is the list's own. A pass that is off is not in it, rather than in
    it and skipped, so what runs is what this returns.
    """
    every: list[MIRTransform] = [
        Fold(where),
        Decide(where),
        Dead(),
        Segments(where),
        Hoist(where),
        Forward(where),
        DropLoads(where),
        DropStores(where),
        Widen(),
        Place(),
    ]
    return [one for one in every if wanted.get(one.name, True)]


# The order, from the pipeline itself rather than beside it: two lists that
# have to agree are one that will not.
PASSES = tuple(one.name for one in pipeline(Where()))


def applied(
    body: MirBody,
    dgroup: frozenset[int],
    calls: dict[int, str],
    *,
    blocks: list | None = None,
    found=None,
    widen: bool = True,
    segments_: bool = True,
    hoist: bool = True,
    forward: bool = True,
    drop_loads: bool = True,
    drop_stores: bool = True,
    place: bool = False,
    only: str | None = None,
) -> MirBody:
    """Every transform this module has, or the one `only` names.

    The absorb pass is gone. It emitted machine operations for the four
    long-arithmetic runtime calls, which is the machine arm's job and
    calls.py's already: the pass ran only under `--no-absorb-calls`, so
    with the default settings it found nothing and no gate exercised it.
    298 lines and 65 of this file's machine references went with it, and
    the plan's answer -- absorption at the raise -- is what replaces it.

    `place` is off for its own reason: sinking a definition is the only
    transform here that changes the order instructions run in, and its
    benefit is indirect.
    """
    wanted = {
        "fold": True,
        "decide": True,
        "dead": True,
        "segments": segments_,
        "hoist": hoist,
        "forward": forward,
        "drop_loads": drop_loads,
        "drop_stores": drop_stores,
        "widen": widen,
        "place": place,
    }
    where = Where(
        dgroup=dgroup,
        calls=calls,
        bounds=module.landmarks(found) if found is not None else None,
        blocks=blocks,
        found=found,
    )
    for one in pipeline(where, **wanted):
        if only is not None and one.name != only:
            continue
        body = one.transform(body)
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


