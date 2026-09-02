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

from qbopt import ir
from qbopt import mir
from qbopt import module
from qbopt import wide
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


# A multiply the 386 can do without multiplying. `lea` through the SIB scale
# is one instruction and several times faster than `imul`, and `shl` is
# smaller as well as faster. gcc and clang at -O3 pick exactly these on an
# i386 -- times three is one lea, not a shift and an add.
BY_LEA = {3: 2, 5: 4, 9: 8}


def _power_of_two(value: int) -> int | None:
    return value.bit_length() - 1 if value > 1 and not value & (value - 1) else None


def strength(body: MirBody, blocks: list | None = None) -> MirBody:
    """A multiply by a constant, as the shift or the address it really is.

    Only where nothing reads the flags afterwards. `lea` writes none at all
    and `shl` writes a different set from `imul`, so a site whose flags are
    read has to keep the multiply -- the same gate absorption applies, asked
    the same way.

    This is a pass rather than a branch inside the absorbed-call emitter on
    purpose: absorption runs in an earlier round, so by the time this looks
    the body has been raised again and `imul eax,3` is an ordinary operation
    with an immediate, whatever wrote it.
    """
    if blocks is None:
        return body
    live = flags.live_in(blocks)
    ends = {one.at: one.end for block in blocks for one in block.insns}

    out = []
    for block in body.blocks:
        ops: list[Op] = []
        for op in block.ops:
            what = op.made if op.made is not None else getattr(op.node, "semantics", None)
            made = _reduced(what)
            if made is None or op.at not in ends or _flags_after(blocks, live, op.at, ends[op.at]) & flags.ALL:
                ops.append(op)
                continue
            ops.append(replace(op, op=made.op, name=made.name or "", made=made))
        out.append(replace(block, ops=tuple(ops)))
    return replace(body, blocks=tuple(out))


def _reduced(what) -> "ir.Semantics | None":
    """`imul r,imm` as a lea or a shift, or None where it is neither."""
    if what is None or what.op is not ir.Operation.MULTIPLY:
        return None
    if len(what.dests) != 1 or len(what.sources) < 2:
        return None
    into, times = what.dests[0], what.sources[-1]
    if not isinstance(into, ir.Reg) or not isinstance(times, ir.Imm):
        return None
    if not isinstance(what.sources[0], ir.Reg) or what.sources[0].register != into.register:
        return None

    if (scale := BY_LEA.get(times.value)) is not None:
        where = ir.Address(None, through=into.register, index=into.register, scale=scale)
        return ir.Semantics(ir.Operation.ADDRESS, "lea", dests=(into,), sources=(where,))
    if (shift := _power_of_two(times.value)) is not None:
        return ir.Semantics(
            ir.Operation.BINARY, "shl", dests=(into,), sources=(into, ir.Imm(value=shift, width=1))
        )
    return None


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
    served = {one.at: one.root for one in avail.forwardable(body, dgroup, calls, want)}
    if not served:
        return body

    out = []
    for block in body.blocks:
        ops: list[Op] = []
        for op in block.ops:
            register = served.get(op.at)
            what = op.made if op.made is not None else getattr(op.node, "semantics", None)
            made = _served(what, register) if register is not None else None
            ops.append(op if made is None else replace(op, made=made, loads=()))
        out.append(replace(block, ops=tuple(ops)))
    return replace(body, blocks=tuple(out))


def _served(what, register) -> "ir.Semantics | None":
    """`what` with its one memory source read from `register` instead."""
    if what is None:
        return None
    cells = [one for one in what.sources if isinstance(one, ir.Mem)]
    if len(cells) != 1:
        return None
    cell = cells[0]
    named = _at_width(register, cell.width)
    if named is None:
        return None
    swapped = tuple(ir.Reg(register=named, width=cell.width) if one is cell else one for one in what.sources)
    return replace(what, sources=swapped)


SEGMENT_REGISTERS = frozenset(
    getattr(Register, one) for one in ("ES", "FS", "GS") if hasattr(Register, one)
)


def _segment_load(op: Op):
    """(register, what it is loaded from) where this op loads a segment."""
    what = op.made if op.made is not None else getattr(op.node, "semantics", None)
    if what is None or what.op is not ir.Operation.MOVE or len(what.dests) != 1 or len(what.sources) != 1:
        return None
    into = what.dests[0]
    if not isinstance(into, ir.Reg) or into.register not in SEGMENT_REGISTERS:
        return None
    return into.register, what.sources[0]


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
    what = op.made if op.made is not None else getattr(op.node, "semantics", None)
    if what is None:
        return True
    if what.op in (ir.Operation.MULTIPLY, ir.Operation.DIVIDE) and len(what.dests) != 1:
        return True  # the widening form, whose dx:ax is not written down
    if what.op is ir.Operation.EXTEND:
        return True
    return (what.name or "") in ("shl", "shr", "sar", "rol", "ror") and any(
        isinstance(one, ir.Reg) and one.register == Register.CL for one in what.sources
    )


def _semantics_of(op: Op):
    return op.made if op.made is not None else getattr(op.node, "semantics", None)


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
    if what is None or op.barrier:
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
            if what.op in (ir.Operation.JUMP, ir.Operation.BRANCH):
                continue
            # Only definition of its register in the loop, or the loop's own
            # later write is what the next pass would see. See _rewritten.
            if any(origin.get(value) in twice for value in one.defines if not value.flags):
                continue
            # Pinning one value recolours the whole body, and an operand
            # nothing writes down does not move with it: hotlop hoisted
            # `mov ax,[n] / imul word [k]`, the recolour renamed the load
            # to cx, and the multiply went on reading ax. It printed 0 for
            # 630. Until an assignment can be constrained to leave these
            # alone, a run holding one is not worth the register.
            if _implicit(one):
                continue
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
    effective = _effective(body, calls)
    reached_by = regalloc._addressing(body)
    addressable = {ir.ROOT.get(one, one) for one in regalloc.ADDRESSING}
    moved: dict[int, list[Op]] = {}
    gone: set[int] = set()
    wanted: dict = {}

    for loop in inside:
        into = _preheader(body, loop)
        if into is None or into in loop.body:
            continue
        ops = [one for at in sorted(loop.body) for one in at_of[at].ops]
        stores = [ref for one in ops for ref in one.stores]
        carried = {phi.result for at in loop.body for phi in at_of[at].phis}
        run = _invariant_run(ops, carried, stores, dgroup, calls, body.origin, bounds)
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
        phis = [phi for at in loop.body for phi in at_of[at].phis]
        crossing = _crossing(run, rest, phis, effective)
        if crossing is None:
            continue

        across = {
            body.origin.get(value)
            for at in (into, *loop.body)
            for value in alive.live_out.get(at, ())
            if not value.flags
        }
        touched = set()
        for one in ops:
            what = _semantics_of(one)
            for where in ((*what.dests, *what.sources) if what else ()):
                if isinstance(where, ir.Reg):
                    touched.add(ir.ROOT.get(where.register, where.register))
        # As many as there are registers for, not all or nothing: the rest
        # go back into the loop, with anything in the run that fed only
        # them.
        spare = [where for where in regalloc.AVAILABLE if where not in across and where not in touched]
        if len(crossing) > len(spare):
            run = _pruned(run, set(sorted(crossing, key=lambda one: one.id)[len(spare) :]))
            if not run:
                continue
            rest = [one for one in ops if one not in run]
            crossing = _crossing(run, rest, phis, effective)
            if crossing is None:
                continue

        # A reader whose operand is implicit does not move with a rename:
        # `imul word [b]` multiplies by ax and says so nowhere. Putting the
        # value back before it runs takes a copy, and a copy takes rewriting
        # the reader with it -- one transformation, and it is the
        # allocator's. Refused here until it is.
        if any(_implicit(other) for other in rest for one in crossing if one in other.uses):
            continue

        # A value some instruction reaches a cell by can only live where
        # 16-bit addressing can reach one -- bx, si or di.
        here = {}
        for result in sorted(crossing, key=lambda one: one.id):
            free = [
                where
                for where in spare
                if result not in reached_by or ir.ROOT.get(where, where) in addressable
            ]
            if not free:
                break
            here[result] = free
        if len(here) != len(crossing):
            continue
        wanted.update(here)
        moved[into] = moved.get(into, []) + list(run)
        gone.update(one.at for one in run)

    if not gone:
        return body

    out = []
    for block in body.blocks:
        ops = [one for one in block.ops if one.at not in gone]
        if block.ops and block.ops[0].at in gone and ops:
            ops = [replace(ops[0], at=block.ops[0].at)] + ops[1:]
        if block.at in moved:
            what = _semantics_of(ops[-1]) if ops else None
            leaves = what is not None and what.op in (ir.Operation.JUMP, ir.Operation.BRANCH)
            here = [replace(one, at=ops[-1].at) for one in moved[block.at]] if ops else moved[block.at]
            ops = (ops[:-1] + here + ops[-1:]) if leaves else (ops + here)
        out.append(replace(block, ops=tuple(ops)))
    laid = replace(body, blocks=tuple(out))
    if not wanted:
        return laid

    # Every candidate, not the first that looks free. A pin displaces
    # whatever held that register, and the displacement can reach a class a
    # phi ties together and that nothing can move. Asking the allocator is
    # the only way to find out, and it is cheap next to a round trip through
    # emission.
    # In SSA again before asking anything about it. The ops have moved, so
    # the phis raised with the original body no longer describe it: a load
    # hoisted out of a loop was loop-carried and is now live once ahead of
    # it, and colour() was being asked about the shape the code had before
    # this pass touched it. rewrite.py re-raises between passes through
    # emission; those bytes do not exist yet here.
    fresh = mir.resolved(laid, calls)
    if isinstance(fresh, str):
        return body
    over = _renamed(laid, fresh, set(wanted))
    if over is None:
        return body
    for choice in _choices(wanted):
        pins = {over[value]: where for value, where in choice.items()}
        if not isinstance(regalloc.colour(fresh, pins), str):
            return replace(laid, pins=choice)
    return body


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
PASSES = ("segments", "hoist", "forward", "drop_loads", "drop_stores", "widen", "place", "absorb", "strength")


def applied(
    body: MirBody,
    dgroup: frozenset[int],
    calls: dict[int, str],
    *,
    blocks: list | None = None,
    absorb: bool = False,
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

    `absorb` is off by default and not because it is unsound: calls.py has
    already taken every arithmetic call before a body reaches here, so with
    it on the MIR emitter finds nothing and no gate exercises it.
    `--no-absorb-calls` is the lever that makes the two comparable.

    `place` is off for its own reason: sinking a definition is the only
    transform here that changes the order instructions run in, and its
    benefit is indirect.
    """
    wanted = {
        "segments": segments_,
        "hoist": hoist,
        "forward": forward,
        "drop_loads": drop_loads,
        "drop_stores": drop_stores,
        "widen": widen,
        "place": place,
        "absorb": absorb and blocks is not None,
        "strength": blocks is not None,
    }
    for name in PASSES:
        if only is not None and name != only:
            continue
        if not wanted[name]:
            continue
        if name == "hoist":
            body = hoisted(body, dgroup, calls, module.landmarks(found) if found is not None else None)
        elif name == "segments":
            body = segments(body, dgroup, calls)
        elif name == "forward":
            body = forwarded(body, dgroup, calls)
        elif name == "drop_loads":
            body = without_redundant_loads(body, dgroup, calls)
        elif name == "drop_stores":
            body = without_dead_stores(body, dgroup, calls)
        elif name == "widen":
            body = widened(body)
        elif name == "place":
            body = placed(body)
        elif name == "absorb":
            body = absorbed(body, blocks or [], calls, found)
        elif name == "strength":
            body = strength(body, blocks)
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


def _wide(register: Register_) -> ir.Reg:
    return ir.Reg(register=register, width=4)


def _narrow(register: Register_) -> ir.Reg:
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
        if isinstance(other, ir.Imm):
            # idiv has no immediate form, so the divisor goes through a
            # register -- before the cdq, which writes edx and would be
            # undone by nothing here but reads better in this order.
            # stride and lngmix are the first programs to divide by a
            # constant, and were refused outright until they existed.
            ecx = _wide(INTO[1])
            steps.append(made(ir.Operation.MOVE, "mov", (ecx,), (other,)))
            other = ecx
        steps.append(made(ir.Operation.EXTEND, "cdq", (edx,), (eax,)))
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
    end = after.node.insn.end if isinstance(after.node, (ir.Opaque, ir.Long, ir.Call)) else at
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
