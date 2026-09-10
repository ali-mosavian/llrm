"""
Which value holds a cell's contents, across blocks.

A forward dataflow whose fact is

    MemRef -> Value

meaning "these bytes are this SSA value". Keyed on MemRef rather than on
Addr because MemRef carries the SSA values its own address is reached
through, so the fact survives anything that reallocates registers -- which
is the whole reason to state it over MIR instead of over machine code.

An entry identifies an SSA value, not a physical register. Forwarding adds
a use and extends that value's lifetime; allocation preserves or spills it
as needed. Requiring it to be live already would retain BC's statement-local
lifetimes instead of optimizing them.

**It intersects at joins rather than placing a phi.** Where predecessors
disagree about which value a cell holds, the fact is dropped. A phi would
be the stronger answer, and mir.py already has the machinery -- but a phi
for a memory cell is a new value with no defining instruction, which
lowering has no way to emit. Conservative here is the honest floor.
"""

from dataclasses import dataclass, replace


from qbopt.model import ir
from qbopt.model import mir
from qbopt.model.mir import Held
from qbopt.model.mir import Kind
from qbopt.model.mir import Op
from qbopt.abi import runtime
from qbopt.model.mir import Value
from qbopt.model.mir import MemRef
from qbopt.model.mir import MirBody
from qbopt.objectfile.module import Space

# What a cell maps to, and the whole lattice element.
Holders = dict[MemRef, Value]


@dataclass(frozen=True, slots=True)
class Held:
    """The map on entry to and exit from each block."""

    into: dict[int, Holders]
    outof: dict[int, Holders]


def _addressing(op: Op) -> set[Value]:
    """Values used only to reach an operand, not read as data."""
    found: set[Value] = set()
    for ref in op.loads + op.stores:
        if ref.base is not None:
            found.add(ref.base)
        if ref.segment is not None:
            found.add(ref.segment)
    return found


def _real(values: tuple[Value, ...]) -> list[Value]:
    return [one for one in values if not one.flags]


def loaded_into(op: Op) -> tuple[MemRef, Value] | None:
    """The cell this op purely loads, and the value it lands in.

    Purely: one read, no write, one value defined, nothing read as data, and
    an address something can name -- a MemRef whose addr is None aliases
    everything, so it can never be matched by same_bytes and would sit in
    the map as an entry no lookup can use and every store has to step over.
    `and cx,[x]` fails the last test -- it uses cx as data as well as
    defining it, so the bytes it leaves in cx are not the cell's. Treating
    it as a load is the bug tools/matrix.py caught in forward.py, and the
    same shape has to be refused here.
    """
    if op.floating is not None or len(op.loads) != 1 or op.stores or op.barrier or op.loads[0].addr is None:
        return None
    defines = _real(op.defines)
    if len(defines) != 1:
        return None
    reading = set(_real(op.uses)) - _addressing(op) - _preserved(op)
    if reading:
        return None
    return op.loads[0], defines[0]


def _preserved(op: Op) -> set[Value]:
    """The uses that are only the destination's own untouched halves.

    A sixteen-bit load writes half of a thirty-two bit variable, so the
    high half survives and MIR records a read of the old value. That read
    is real -- refusing to model it would be wrong -- but it is not the
    operation consulting memory's contents, which is the question
    loaded_into asks. Without this, every one of the 36 loads forward.py
    deletes came back "not a plain load", and none of them was anything
    else.

    **The operation has to be a load.** A subtract from a cell reads the
    old value the same way, and there the read is the whole point: what is
    left is not the cell's. The two are told apart by the operands -- a
    load's only argument is the cell, so any other use is a preserved half,
    while a subtract names its other input among its arguments. This asked
    the instruction whether it was a MOVE and looked the register up in
    `origin`; leaving it out deleted `sub ax,ds:[0]` and `adc dx,[si+2]`
    from a generated program.
    """
    if op.kind is not Kind.LOAD:
        return set()
    named = {one.value for one in op.args if isinstance(one, Held)}
    return {one for one in _real(op.uses) if one not in named}


def stored_from(op: Op) -> tuple[MemRef, Value] | None:
    """The cell this op purely stores, and the value it wrote there."""
    if op.floating is not None or len(op.stores) != 1 or op.loads or op.barrier or op.stores[0].addr is None:
        return None
    if _real(op.defines):
        return None
    reading = [one for one in _real(op.uses) if one not in _addressing(op)]
    if len(reading) != 1:
        return None
    return op.stores[0], reading[0]


def _covered_by(ref: MemRef, other: MemRef, dgroup: frozenset[int]) -> bool:
    """Whether a later store through `other` writes every byte of `ref`.

    Not `same_bytes`, which asks whether the two name the same cell. A long
    written as one `mov [x],eax` covers both halves BC stored separately,
    and asking for equality missed the second of them -- which is most of
    what memory.py was finding and this was not, since absorption emits
    exactly that shape.
    """
    ref, other = mir._symbolic_ref(ref), mir._symbolic_ref(other)
    if ref.addr is None or other.addr is None:
        return False
    aligned = replace(other, addr=other.addr.plus(ref.addr.disp - other.addr.disp), width=ref.width)
    if not mir.same_bytes(ref, aligned):
        return False
    return other.addr.disp <= ref.addr.disp and ref.addr.disp + ref.width <= other.addr.disp + other.width


def stored_cell(op: Op) -> MemRef | None:
    """The cell this op purely stores to, whatever it put there.

    stored_from() answers a narrower question -- the cell *and the value* --
    and needs exactly one value read to name the second. `mov word [x],1`
    reads none, so it came back None and a store of a constant was invisible
    to the dead-store walk. Which cell was written is the whole question
    there; what was written is not.
    """
    if op.floating is not None or len(op.stores) != 1 or op.loads or op.barrier or op.stores[0].addr is None:
        return None
    if _real(op.defines):
        return None
    return op.stores[0]


def _clean(op: Op, calls: dict[int, str]) -> bool:
    """Whether this call provably leaves caller memory alone.

    mir.py gives a call a store of `MemRef(addr=None)`, which aliases
    everything and wipes this map. That is the right default and the wrong
    answer for the routines runtime.py has actually read: B$MUI4 multiplies
    two longs in registers and touches no caller memory at all, so a cell
    established before it is still that value after.

    Registers are a separate question and are not answered here. A call
    clobbers physical registers independently of memory. An SSA value remains
    the same value; allocation must preserve it if forwarding extends its use.
    """
    name = calls.get(op.at)
    if name is None:
        return False
    routine = runtime.contract(name)
    return routine.established and not runtime.barrier(routine) and not runtime.writes_caller_memory(routine)


def _after(
    op: Op,
    holders: Holders,
    dgroup: frozenset[int],
    calls: dict[int, str],
) -> Holders:
    """The map across one op."""
    if op.barrier:
        return {}
    if op.at in calls:
        # Even a call runtime.py proves memory-clean uses the stack: it is
        # entered by a push of the return address and the callee pops its
        # own arguments off. "Writes no caller memory" is a claim about the
        # caller's variables, never about the scratch below sp.
        return _local(holders) if _clean(op, calls) else {}

    for ref in op.stores:
        holders = {one: who for one, who in holders.items() if not mir.overlapping(one, ref, dgroup)}
    found = stored_from(op) or loaded_into(op)
    if found is not None:
        ref, value = found
        holders = dict(holders)
        holders[ref] = value
    return holders


def _local(holders: Holders) -> Holders:
    """Without the stack slots, which do not survive a block boundary.

    A Space.STACK address is a depth measured from the top of the block that
    pushed it, so `[sp-8]` in one block and `[sp-8]` in another are two
    different addresses that compare equal. Carrying one across an edge is
    the one way this analysis could be unsound, so it does not.
    """
    return {one: who for one, who in holders.items() if one.addr is None or one.addr.space is not Space.STACK}


def _meet(maps: list[Holders]) -> Holders:
    """Only what every predecessor agrees on, value and all."""
    if not maps:
        return {}
    out = _local(maps[0])
    for other in maps[1:]:
        kept = _local(other)
        out = {one: who for one, who in out.items() if kept.get(one) == who}
    return out


def holders(
    body: MirBody,
    dgroup: frozenset[int],
    calls: dict[int, str] | None = None,
    partial: bool = False,
) -> Held:
    """Which value each cell holds, at every block's entry and exit.

    `partial` also records a cell loaded by a *partial* write --
    `mov ax,[x]`, which writes sixteen bits of a thirty-two bit variable
    and leaves the high half alone. Off by default, and the default is
    the one that matters: such an entry names a 32-bit value that holds
    the cell only in its low half, which is exactly right for deciding
    the load is a no-op and wrong for serving some other read from that
    register. redundant() asks for it; forwardable() must not, and a
    generated program caught it doing so.
    """
    calls = calls or {}
    preds: dict[int, list[int]] = {block.at: [] for block in body.blocks}
    for block in body.blocks:
        for succ in block.succ:
            if succ in preds:
                preds[succ].append(block.at)

    into: dict[int, Holders] = {block.at: {} for block in body.blocks}
    outof: dict[int, Holders] = {block.at: {} for block in body.blocks}

    changing = True
    while changing:
        changing = False
        for block in body.blocks:
            arriving = {} if block.at == body.entry else _meet([outof[one] for one in preds[block.at]])
            leaving = dict(arriving)
            for op in block.ops:
                leaving = _after(op, leaving, dgroup, calls)
            if arriving != into[block.at] or leaving != outof[block.at]:
                into[block.at], outof[block.at] = arriving, leaving
                changing = True

    return Held(into, outof)


def provider(
    body: MirBody, dgroup: frozenset[int], at: int, ref: MemRef, calls: dict[int, str] | None = None
) -> Value | None:
    """The value holding `ref`'s bytes just before the op at `at`.

    A new use extends its lifetime; this says nothing about its allocation.
    """
    calls = calls or {}
    found = holders(body, dgroup, calls)
    for block in body.blocks:
        if not any(op.at == at for op in block.ops):
            continue
        current = dict(found.into[block.at])
        for op in block.ops:
            if op.at == at:
                return next((who for one, who in current.items() if mir.same_bytes(one, ref)), None)
            current = _after(op, current, dgroup, calls)
    return None


@dataclass(frozen=True, slots=True)
class Forward:
    """A memory read whose bytes equal a known SSA value."""

    at: int
    # The value holding them. Which register that is, is the allocator's
    # answer; a caller that has no values -- the machine arm -- looks it up
    # in the body's own `origin`, which is where that question belongs.
    value: "mir.Value"
    op: Op | None = None


def _dead_in(block, overwritten: dict, dgroup: frozenset[int], calls: dict[int, str]):
    """One block, backward, from what its successors have already overwritten.

    Returns the stores it found dead and what is overwritten on entry, so
    the caller can carry the second to the predecessors.
    """
    found: list[int] = []
    overwritten = dict(overwritten)
    for op in reversed(block.ops):
        if op.kind is Kind.JOIN:
            # `push eax / pop ax / pop dx`. A barrier for values, because
            # nothing here can name in SSA what it writes -- and nothing at
            # all for memory: ir.RESTORE_EFFECTS reports no load and no
            # store, since sp comes back where it started and nothing
            # outside the idiom reads the cells it passed through.
            continue
        if op.floating is not None or op.kind is Kind.FCHECK or op.barrier or (op.at in calls and not _clean(op, calls)):
            overwritten = {}
            continue
        if op.at in calls:
            # A clean call still carries mir.py's own MemRef(addr=None) in
            # both loads and stores -- the default that aliases everything
            # -- so falling through to the clearing below wiped the map for
            # a routine _clean() had just proved touches no caller memory.
            # Its arguments are on the stack, and a stack cell is never in
            # this map to begin with.
            continue

        wrote = stored_cell(op)
        if wrote is not None:
            ref = wrote
            if ref.addr is not None and ref.addr.space is not Space.STACK:
                if any(_covered_by(ref, one, dgroup) for one in overwritten):
                    found.append(id(op))
                overwritten[ref] = op.at
                continue

        # Anything this op reads, and anything it writes that this
        # cannot name, puts the cells it may touch back in doubt.
        for ref in op.loads:
            if ref.addr is not None and ref.addr.space is Space.STACK:
                continue  # a pop, for the same reason a push is skipped below
            overwritten = {one: at for one, at in overwritten.items() if not mir.overlapping(one, ref, dgroup)}
        if wrote is None:
            for ref in op.stores:
                if ref.addr is not None and ref.addr.space is Space.STACK:
                    # A push. `stored_from()` does not name it, so without
                    # this it clears through the general case -- and
                    # may_alias() says a stack cell and a static may be the
                    # same byte, because BC runs with SS == DS. True only of
                    # a program whose stack has already grown down into its
                    # own data, which has crashed. Four pushes ahead of a
                    # call were wiping everything known, which is where the
                    # seven stores memory.py finds and this did not all sat.
                    continue
                overwritten = {one: at for one, at in overwritten.items() if not mir.overlapping(one, ref, dgroup)}
    return found, overwritten


def dead_stores(body: MirBody, dgroup: frozenset[int], calls: dict[int, str]) -> tuple[Op, ...]:
    """Stores whose bytes are overwritten before anything reads them.

    Return operations, not addresses: inserted stores can share an address
    with an unrelated live load.

    memory.py's own pass, restated over MIR. Backward through each block:
    a store to a cell that a later store overwrites, with nothing in
    between that could have read it, computed nothing.

    Across edges, not only within a block -- a cell has to be overwritten on
    *every* successor path, so what a block starts from is the intersection
    of what its successors have. That is a must-analysis, and the fixed
    point starts from "nothing is overwritten" and grows: the conservative
    direction, and it means a cycle cannot justify itself into judging a
    store dead that is not. Block-scoped was costing seven stores against
    memory.py, all of them BC's module init, where the overwrite is in a
    later block.

    A block with no successor, or one this cannot see, starts from nothing:
    the caller may read the cell.

    These clear what is known, and each is a way the cell could be
    read without this seeing a load of it: a barrier, whose addresses are
    its own; a call runtime.py has not proved leaves caller memory alone;
    a strict floating operation whose exception handler can observe memory;
    and a load that may alias, which is the ordinary case.

    Stack slots are excluded outright rather than reasoned about. A
    Space.STACK address is a depth from the top of its own block and a
    store to one is an argument something is about to consume, so "nothing
    read it" is a claim this has no standing to make.
    """
    known = {block.at: block for block in body.blocks}
    entry: dict[int, dict] = {at: {} for at in known}
    found: set[int] = set()

    for _round in range(len(known) + 1):
        changing = False
        for block in sorted(body.blocks, key=lambda one: one.at, reverse=True):
            out: dict | None = None
            for successor in block.succ:
                have = entry.get(successor)
                if have is None:  # an edge out of this body
                    out = {}
                    break
                if out is None:
                    out = dict(have)
                else:
                    out = {one: at for one, at in out.items() if any(mir.same_bytes(one, other) for other in have)}
            if out is None:
                out = {}  # no successor at all: the caller may read it
            mine, start = _dead_in(block, out, dgroup, calls)
            found.update(mine)
            if len(start) != len(entry[block.at]):
                changing = True
            entry[block.at] = start
        if not changing:
            break
    return tuple(op for block in body.blocks for op in block.ops if id(op) in found)


def redundant(
    body: MirBody, dgroup: frozenset[int], calls: dict[int, str]
) -> tuple[tuple[int, Value, Value], ...]:
    """Loads that put back into a register exactly what it already held.

    `(where, what the load defined, the value that already held it)`. The
    third is the point: deleting the load removes the only definition of
    the second, and every later reader has to be told to read the provider
    instead. This used to return the address alone, so the caller deleted
    the instruction and substituted nothing -- arridx's add went on naming
    a value nobody wrote, allocation gave that phantom a register of its
    own, and the program answered 0 where it should answer 1260.

    forward.py's deletion, restated over MIR. `mov ax,[x]` where ax already
    holds [x] computes nothing, so removing it leaves every later
    instruction reading what it expected -- the same argument the machine
    pass makes, over values rather than over a backward scan of registers.

    The two are not the same question as forwardable()'s. That one serves a
    read from a *different* register and substitutes the operand; this one
    finds a read whose answer is already in its own destination and drops
    the instruction. An accumulate can never be one of these: `and cx,[x]`
    does not leave [x] in cx, and loaded_into refuses it for that reason.

    Whole registers only. `mov ax,[x]` writes half of eax and the other half
    survives, which is exactly why the deletion is safe -- the instruction
    is a no-op, so the preserved half is preserved either way -- but the
    holder has to be the same variable BC would have read, not a wider one
    that merely contains it. loaded_into's own _preserved() is what lets
    this see the load at all.
    """
    held = holders(body, dgroup, calls, partial=True)
    found: list[tuple[int, Value, Value]] = []
    for block in body.blocks:
        current = dict(held.into[block.at])
        # Which value each register actually holds right now. holders() is a
        # map from a cell to the value that was put there, and it says
        # nothing about whether that value is still in its register: after
        # `mov ax,[x]` then `mov ax,[y]`, the entry for [x] still names a
        # value whose origin is eax, and eax holds [y]. Deleting a later
        # `mov ax,[x]` on the strength of that entry is how this read the
        # wrong cell. SSA forwarding instead retains the load's definition
        # and replaces its memory operand with the known value.
        inside: dict[Register_, Value] = {}
        for op in block.ops:
            # A call clobbers ax, cx, dx and bx whatever it does to memory,
            # so a value that was in one of them is not there afterwards.
            # _after() keeps the *memory* map across a call runtime.py has
            # proved clean, which is right and is exactly what makes this
            # separate bookkeeping necessary: the cell is still that value
            # and the register is not.
            if op.at in calls or op.barrier:
                inside = {}
                continue
            got = loaded_into(op)
            if got is not None:
                ref, made = got
                into = body.origin.get(made)
                who = next((w for cell, w in current.items() if mir.same_bytes(cell, ref)), None)
                if who is not None and into is not None and body.origin.get(who) is into and inside.get(into) is who:
                    found.append((op.at, made, who))
            for value in op.defines:
                where = body.origin.get(value)
                if where is not None:
                    inside[where] = value
            current = _after(op, current, dgroup, calls)
    return tuple(found)


def forwardable(
    body: MirBody,
    dgroup: frozenset[int],
    calls: dict[int, str],
    want: frozenset[int],
) -> tuple[Forward, ...]:
    """Reads in `want` that a known SSA value can serve instead of memory.

    The caller replaces the operand, retaining any arithmetic and extending
    the provider's lifetime. Operation identity distinguishes captures that
    share a source address.
    """
    held = holders(body, dgroup, calls)
    found: list[Forward] = []

    for block in body.blocks:
        current = dict(held.into[block.at])
        for op in block.ops:
            if op.at in want and op.loads:
                who = next((w for cell, w in current.items() if mir.same_bytes(cell, op.loads[0])), None)
                if who is not None:
                    found.append(Forward(op.at, who, op))
            current = _after(op, current, dgroup, calls)
    return tuple(found)
