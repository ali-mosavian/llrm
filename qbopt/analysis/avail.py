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

This lattice intersects at joins. Where predecessor values differ,
`optimize/loadjoins.py` can form a value phi using MemorySSA's per-edge
availability proof. Lowering and allocation handle that phi normally.
"""

from dataclasses import field
from dataclasses import replace
from dataclasses import dataclass
from collections.abc import Callable

from qbopt.model import mir
from qbopt.model import memory
from qbopt.model.mir import Op
from qbopt.analysis import loops
from qbopt.analysis import cellmap
from qbopt.model.mir import Kind
from qbopt.model.mir import Value
from qbopt.analysis import effects
from qbopt.model.mir import MemRef
from qbopt.model.mir import MirBody
from qbopt.analysis import memoryssa
from qbopt.objectfile.module import Space

# What a cell maps to, and the whole lattice element.
Holders = dict[MemRef, Value]


@dataclass(frozen=True, slots=True)
class Held:
    """The map on entry to and exit from each block."""

    into: dict[int, Holders]
    outof: dict[int, Holders]
    # Each value's constant, so a far access through a literal selector is
    # not taken to reach the frame.
    known: dict = field(default_factory=dict)


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

    Purely: a LOAD, one read, no write, one value defined, nothing read as data, and
    an address something can name, either a symbolic cell or a whole SSA
    pointer. An anonymous memory effect is neither and cannot supply data.
    `and cx,[x]` fails the last test -- it uses cx as data as well as
    defining it, so the bytes it leaves in cx are not the cell's. Treating
    it as a load is the bug tools/matrix.py caught in forward.py, and the
    same shape has to be refused here. Constants are not SSA uses: after
    folding, `20 + [base]` can have only address uses yet is still not a load.
    """
    if (
        op.kind is not Kind.LOAD
        or op.floating is not None
        or len(op.loads) != 1
        or op.stores
        or op.barrier
        or op.loads[0].addr is None
        and not op.loads[0].pointer
    ):
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
    named = {one.value for one in op.args if isinstance(one, mir.Held)}
    return {one for one in _real(op.uses) if one not in named}


def stored_from(op: Op) -> tuple[MemRef, Value] | None:
    """The cell this op purely stores, and the value it wrote there.

    Purely: the cell receives the value, not something computed from it.
    `inc [x]` and `add [x],1` read one value and store another, and naming
    the one they read made deedlines' zoom read `kxy0%` as the value it
    held before the branch that changed it.
    """
    if (
        op.kind not in (Kind.STORE, Kind.ARG)
        or op.floating is not None
        or len(op.stores) != 1
        or op.loads
        or op.barrier
        or op.stores[0].addr is None
        and not op.stores[0].pointer
    ):
        return None
    if _real(op.defines):
        return None
    reading = [one for one in _real(op.uses) if one not in _addressing(op)]
    if not reading:
        # A constant written straight to the cell reads no value at all, so
        # the count above refused it and the bytes were unknowable for the
        # rest of the body. `DEF SEG = &HA000` is one store and twenty-three
        # reads, none of which could be served.
        written = [one for one in op.args if isinstance(one, (mir.Const, mir.Symbol))]
        if len(written) == 1 and len(op.args) == 1 and written[0].width == op.stores[0].width:
            return op.stores[0], written[0]
        return None
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


def _after(
    op: Op,
    holders: Holders,
    dgroup: frozenset[int],
    calls: dict[int, str],
    known: dict | None = None,
) -> Holders:
    """The map across one op."""
    if effects.unmodeled_write(op):
        return {}
    if op.kind is Kind.CALL:
        holders = _local(holders)

    for ref in op.stores:
        holders = {
            one: who
            for one, who in holders.items()
            if not mir.overlapping(one, ref, dgroup, known=known, other_known=known)
        }
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
) -> Held:
    """Which value each cell holds, at every block's entry and exit."""
    from qbopt.analysis import ranges

    calls = calls or {}
    known = ranges.constants(body, dgroup, calls)
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
                leaving = _after(op, leaving, dgroup, calls, known)
            if arriving != into[block.at] or leaving != outof[block.at]:
                into[block.at], outof[block.at] = arriving, leaving
                changing = True

    return Held(into, outof, known)


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
            current = _after(op, current, dgroup, calls, found.known)
    return None


@dataclass(frozen=True, slots=True)
class Forward:
    """A memory read whose bytes equal a known SSA value, or a constant."""

    at: int
    # The value holding them. Which register that is, is the allocator's
    # answer; this analysis never asks where either value used to live.
    value: "mir.Value | mir.Const | mir.Symbol"
    op: Op | None = None


def _fixed(ref: MemRef) -> bool:
    """Whether a cell is named outright rather than reached through a value.

    A direct symbolic operand names its bytes.  So does canonical provenance:
    an SSA pointer may choose the machine address at run time while its object
    and possible byte lanes are already explicit in MIR.  An unresolved
    pointer, index or selector remains unnamed and reaches a private cell no
    more than an unknown call does.
    """
    canonical = (
        ref.provenance is not None
        and bool(ref.provenance.slices)
        and all(one.object.kind is not memory.Kind.UNKNOWN for one in ref.provenance.slices)
    )
    direct = ref.addr is not None and ref.addr.direct and ref.base is None and ref.segment is None
    return canonical or direct


def _dead_in(
    block: mir.MirBlock,
    overwritten: dict[MemRef, int],
    dgroup: frozenset[int],
    calls: dict[int, str],
    private: "Callable[[MemRef], bool] | None" = None,
    bounds: dict | None = None,
    sealed: bool = False,
    handles_errors: bool = True,
) -> tuple[list[int], dict[MemRef, int]]:
    """One block, backward, from what its successors have already overwritten.

    Returns the stores it found dead and what is overwritten on entry, so
    the caller can carry the second to the predecessors.
    """
    found: list[int] = []
    overwritten = _cells(overwritten)
    for op in reversed(block.ops):
        # Nothing can read a private cell but by its name: not a call, and
        # not an address this cannot resolve.
        shielded = private is not None and op.floating is None and not op.barrier
        if op.kind is Kind.JOIN:
            # `push eax / pop ax / pop dx`. A barrier for values, because
            # nothing here can name in SSA what it writes -- and nothing at
            # all for memory: ir.RESTORE_EFFECTS reports no load and no
            # store, since sp comes back where it started and nothing
            # outside the idiom reads the cells it passed through.
            continue
        exception = effects.exposes_memory(op, handles_errors)
        if exception or effects.unmodeled_write(op) or effects.unmodeled_read(op):
            # A float exception's handler runs outside the body, and in a
            # sealed one resumes nowhere inside: a private cell is as safe as
            # across a call, and the op's own cells are read as any op's.
            caught = exception and sealed and private is not None and not op.barrier and not effects.unmodeled_write(op)
            kept = (shielded and op.kind is Kind.CALL) or caught
            overwritten = _cells({one: at for one, at in overwritten.items() if private(one)} if kept else {})
            if not caught:
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
            _clobber(overwritten, ref, shielded and (op.kind is Kind.CALL or not _fixed(ref)), private, dgroup, bounds)
        if wrote is None:
            for ref in op.stores:
                if ref.addr is not None and ref.addr.space is Space.STACK:
                    # A push. `stored_from()` does not name it, so without
                    # this it clears through the general case -- and
                    # regions says a stack cell and a static may be the
                    # same byte, because BC runs with SS == DS. True only of
                    # a program whose stack has already grown down into its
                    # own data, which has crashed. Four pushes ahead of a
                    # call were wiping everything known, which is where the
                    # seven stores memory.py finds and this did not all sat.
                    continue
                _clobber(overwritten, ref, shielded and (op.kind is Kind.CALL or not _fixed(ref)), private, dgroup, bounds)
    return found, overwritten


def _cells(overwritten: dict[MemRef, int]) -> cellmap.CellMap:
    """A copy of `overwritten` to change, bucketed as ``mir.overlapping`` rules writes out."""
    if isinstance(overwritten, cellmap.CellMap):
        return overwritten.copy()
    return cellmap.CellMap(mir.overlap_bucket, overwritten)


def _clobber(overwritten: cellmap.CellMap, ref: MemRef, unnamed: bool, private, dgroup, bounds) -> None:
    """Forget the cells an access through `ref` may touch; an `unnamed` one cannot reach a private cell."""
    overwritten.kill(mir.overlap_buckets(ref, overwritten), lambda one: not (unnamed and private(one)) and mir.overlapping(one, ref, dgroup, bounds))


def dead_stores(
    body: MirBody,
    dgroup: frozenset[int],
    calls: dict[int, str],
    private: "Callable[[MemRef], bool] | None" = None,
    bounds: dict | None = None,
    handles_errors: bool = True,
) -> tuple[Op, ...]:
    """Stores whose bytes are overwritten before anything reads them.

    Return operations, not addresses: inserted stores can share an address
    with an unrelated live load.

    memory.py's own pass, restated over MIR. Backward through each block:
    a store to a cell that a later store overwrites, with nothing in
    between that could have read it, computed nothing.

    Across edges, not only within a block -- a cell has to be overwritten on
    *every* successor path, so what a block starts from is the intersection
    of what its successors have. That is liveness seen from the other side:
    a store is live only if some path reads it, so the fixed point starts
    from every stored cell and shrinks. Starting from nothing and growing
    let a loop's back edge veto every store inside the loop: nbody wrote
    four variables per inner pass that nothing ever read. Only the last
    round's verdicts stand, since an earlier one was made on a guess.
    Block-scoped was costing seven stores against memory.py, all of them
    BC's module init, where the overwrite is in a later block.

    A block with no successor, or one this cannot see, starts from nothing:
    the caller may read the cell -- unless `private` says nothing outside
    the body can, in which case it starts from every such cell stored here.

    These clear what is known, and each is a way the cell could be
    read without this seeing a load of it: a barrier, whose addresses are
    its own; a call runtime.py has not proved leaves caller memory alone;
    an operation that can trap, in a module with an ON ERROR handler;
    and a load that may alias, which is the ordinary case.

    Stack slots are excluded outright rather than reasoned about. A
    Space.STACK address is a depth from the top of its own block and a
    store to one is an argument something is about to consume, so "nothing
    read it" is a claim this has no standing to make.
    """
    known = {block.at: block for block in body.blocks}
    stored = {
        ref: -1
        for block in body.blocks
        for op in block.ops
        if (ref := stored_cell(op)) is not None and ref.addr is not None and ref.addr.space is not Space.STACK
    }
    entry: dict[int, dict] = {at: dict(stored) for at in known}
    unread = (
        {ref: -1 for block in body.blocks for op in block.ops if (ref := stored_cell(op)) is not None and private(ref)}
        if private is not None
        else {}
    )

    changing = True
    while changing:
        changing = False
        found: set[int] = set()
        for block in sorted(body.blocks, key=lambda one: one.at, reverse=True):
            out: dict | None = None
            for successor in block.succ:
                have = entry.get(successor)
                if have is None:  # an edge out of this body
                    out = dict(unread)
                    break
                if out is None:
                    out = dict(have)
                else:
                    out = {one: at for one, at in out.items() if any(mir.same_bytes(one, other) for other in have)}
            if out is None:
                out = dict(unread)  # no successor at all: only the caller may read it
            mine, start = _dead_in(block, out, dgroup, calls, private, bounds, body.sealed, handles_errors)
            found.update(mine)
            if len(start) != len(entry[block.at]):
                changing = True
            entry[block.at] = start
    return tuple(op for block in body.blocks for op in block.ops if id(op) in found)


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
    missing: list[tuple[memoryssa.Site, Op]] = []

    for block in body.blocks:
        current = dict(held.into[block.at])
        for index, op in enumerate(block.ops):
            if op.at in want and op.loads:
                who = next((w for cell, w in current.items() if mir.same_bytes(cell, op.loads[0])), None)
                if who is not None:
                    found.append(Forward(op.at, who, op))
                else:
                    missing.append((memoryssa.Site(block.at, index), op))
            current = _after(op, current, dgroup, calls, held.known)
    if missing:
        found.extend(_memory_providers(body, dgroup, missing))
    return tuple(found)


def _memory_providers(
    body: MirBody,
    dgroup: frozenset[int],
    missing: list[tuple[memoryssa.Site, Op]],
) -> list[Forward]:
    """Recover dominating memory values lost by the forward lattice at loops."""
    graph = memoryssa.built(body)
    accesses = {access.id: access for access in graph.accesses}
    dominators = loops.dominators(body.blocks, body.entry)
    loads = [(site, loaded) for site, op in graph.operations.items() if (loaded := loaded_into(op)) is not None]
    found: list[Forward] = []

    def available(source: memoryssa.Site, site: memoryssa.Site, cell: MemRef, value: Value) -> bool:
        return (
            source.block in dominators[site.block]
            and (source.block != site.block or source.index < site.index)
            and (source.block == site.block or bool(_local({cell: value})))
        )

    for site, op in missing:
        if len(op.loads) != 1 or op.barrier or op.kind is Kind.CALL:
            continue
        clobbers = graph.clobbers(site, op.loads[0], dgroup)
        access = accesses[next(iter(clobbers))] if len(clobbers) == 1 else None
        if access is not None and access.kind is memoryssa.Kind.DEF and access.site is not None:
            stored = stored_from(graph.operations[access.site])
            if stored is not None:
                cell, value = stored
                if available(access.site, site, cell, value) and graph.pointers.same_bytes(cell, op.loads[0]):
                    found.append(Forward(op.at, value, op))
                    continue
        for source, (cell, value) in loads:
            if (
                available(source, site, cell, value)
                and graph.pointers.same_bytes(cell, op.loads[0])
                and graph.unchanged(source, site, cell, dgroup)
            ):
                found.append(Forward(op.at, value, op))
                break
    return found
