"""Write-through promotion: reuse a stored value across statements.

Inspired by LLVM's `mem2reg`. A variable BC keeps in memory and reloads for every
statement -- because it compiles a statement at a time, which is the fact
under every number in `docs/targets.md` -- becomes an SSA value, and the
register allocator can keep that value across statements. Unlike LLVM's
local allocas, these cells can be visible outside this body: every store
stays in place. Removing stores needs a separate proof of observability.

**Which cells.** A fixed data/frame address or a proven indexed data cell,
identified by its address SSA value, with reusable reads at one width.
A read reuses the stored value only when it is available along every
incoming path. Any possibly aliasing write invalidates that availability;
re-executing an address definition invalidates its indexed cells as well.
a later direct store establishes it again. Constant initializers can establish
each fully covered field, including across split stores, without changing the stores.
Runtime calls use the same
memory-effect contracts as the rest of the alias analysis.

Direct loads, stores and selected integer arithmetic reads are supported;
every reused read must have an available definition. Unsupported reads stay
in memory, which is safe because all original stores remain in place.

**No phi is written.** A store also defines a fresh variable and
a load becomes a use of it, and `mir.resolved()` re-derives the SSA -- it
renames per variable and puts a phi wherever two definitions meet. That is
the whole of `mem2reg`'s phi placement, and writing one here would be
saying the same thing twice.
"""

from collections import Counter
from dataclasses import replace

from qbopt.model import mir
from qbopt.analysis import ssa
from qbopt.model.mir import Op
from qbopt.analysis import loops
from qbopt.analysis import consts
from qbopt.analysis import effects
from qbopt.model.mir import MirBody
from qbopt.model.passes import Where
from qbopt.objectfile.module import Space
from qbopt.model.passes import MIRTransform

READS = frozenset(
    {
        mir.Kind.LOAD,
        mir.Kind.ADD,
        mir.Kind.ADD_CARRY,
        mir.Kind.SUB,
        mir.Kind.INCREMENT,
        mir.Kind.DECREMENT,
        mir.Kind.MUL,
        mir.Kind.AND,
        mir.Kind.OR,
        mir.Kind.XOR,
    }
)

CELLS = frozenset({Space.SEGMENT, Space.FRAME})


def _key(ref):
    if ref.addr is None or ref.segment is not None or ref.addr.space not in CELLS:
        return None
    # Keep canonical object identity on the promoted cell. Reducing a direct
    # reference back to Addr here made an explicitly nonlocal call effect
    # meet a provenance-less frame reference, falling through to the legacy
    # conservative query and losing the proof the frontend supplied.
    if ref.provenance is not None:
        return replace(ref, width=0)
    if ref.base is None:
        return ref.addr
    if ref.addr.space is Space.SEGMENT and ref.excludes:
        return replace(ref, width=0, excludes=())
    return None


def _reference(key, width):
    return replace(key, width=width) if isinstance(key, mir.MemRef) else mir.MemRef(key, width)


def _order(key):
    ref = _reference(key, 0)
    return ref.addr.index, ref.addr.disp, -1 if ref.base is None else ref.base.id


class Promote(MIRTransform):
    name = "promote"

    def __init__(self, where: Where) -> None:
        self.where = where

    def transform(self, body: MirBody) -> MirBody:
        return promoted(body, self.where.dgroup, self.where.bounds)


def promotable(body: MirBody, dgroup: frozenset[int] = frozenset(), bounds: dict | None = None) -> dict:
    """Cells whose reads can use a known stored value, by address.

    Touched more than once, because promoting a cell read or written once
    removes no work. Supported reads must agree on one width. Exact stores and
    complete constant initializers can establish that value; arbitrary partial
    writes cannot. Availability excludes reads after intervening aliasing writes.
    """
    every = [one for block in body.blocks for op in block.ops for one in (*op.loads, *op.stores)]
    seen: Counter = Counter()
    widths: dict = {}
    for one in every:
        key = _key(one)
        if key is None:
            continue
        seen[key] += 1
    for block in body.blocks:
        for op in block.ops:
            if op.loads and (ref := _cell(op)) is not None:
                widths.setdefault(_key(ref), set()).add(ref.width)

    candidates = {
        addr: next(iter(widths[addr]))
        for addr, times in seen.items()
        if times > 1 and addr in widths and len(widths[addr]) == 1
    }
    usable = _available(body, candidates, dgroup, bounds)
    used = {_key(ref) for block in body.blocks for op in block.ops if id(op) in usable for ref in op.loads}
    return {addr: width for addr, width in candidates.items() if addr in used}


def _cell(op: Op) -> mir.MemRef | None:
    """The whole access this pass can replace, never part of an operation."""
    match op.kind, op.loads, op.stores:
        case kind, (ref,), () if kind in READS:
            return ref if _key(ref) is not None else None
        case mir.Kind.STORE, (), (ref,):
            return ref if _key(ref) is not None else None
    return None


def _initializers(body: MirBody, cells: dict, dgroup: frozenset[int], bounds: dict | None) -> dict:
    """Complete scalar constants established by intact, possibly split stores."""
    calls = {op.at: "" for block in body.blocks for op in block.ops if op.barrier or op.kind is mir.Kind.CALL}
    memory = consts.cells(body, dgroup, calls)
    initialized = {}
    for block in body.blocks:
        for index, op in enumerate(block.ops):
            cell = _cell(op)
            if (
                op.kind is not mir.Kind.STORE
                or op.barrier
                or cell is None
                or cell.addr is None
                or cell.base is not None
                or cell.segment is not None
                or cell.addr.space not in CELLS
            ):
                continue
            after = consts._kills(memory.get((block.at, index), {}), op, {}, dgroup, calls)
            initialized[id(op)] = {
                addr: fact
                for addr, width in cells.items()
                if not isinstance(addr, mir.MemRef)
                and (addr, width) != (cell.addr, cell.width)
                and mir.overlapping(mir.MemRef(addr, width), cell, dgroup, bounds)
                and (fact := consts._cell(after, mir.MemRef(addr, width))) is not None
            }
    return initialized


def _available(body: MirBody, cells: dict, dgroup: frozenset[int], bounds: dict | None) -> set[int]:
    """Reads reached by a stored value on every path, without an intervening aliasing write."""
    reachable = {at for at, doms in loops.dominators(body.blocks, body.entry).items() if doms}
    predecessors = loops.predecessors(body.blocks)
    leaving = {at: set(cells) for at in reachable}
    refs = {addr: _reference(addr, width) for addr, width in cells.items()}
    initializers = _initializers(body, cells, dgroup, bounds)

    def entering(at: int) -> set:
        parents = predecessors[at] & reachable
        return set.intersection(*(leaving[parent] for parent in parents)) if parents and at != body.entry else set()

    def through(block: mir.MirBlock, available: set, reads: set[int] | None = None) -> set:
        def redefined(values):
            available.difference_update(key for key, ref in refs.items() if ref.base in values or ref.segment in values)

        redefined({phi.result for phi in block.phis})
        for op in block.ops:
            if effects.unmodeled_write(op):
                available.clear()
                continue
            cell = _cell(op)
            key = _key(cell) if cell is not None else None
            if reads is not None and op.loads and cell is not None and key in available and cell.width == cells[key]:
                reads.add(id(op))
            redefined(set(op.defines))
            available.difference_update(
                addr
                for addr, ref in refs.items()
                if any(mir.overlapping(ref, written, dgroup, bounds) for written in op.stores)
            )
            if op.kind is mir.Kind.STORE and cell is not None and key in cells and cell.width == cells[key]:
                available.add(key)
            available.update(initializers.get(id(op), {}))
        return available

    while True:
        changed = False
        for block in body.blocks:
            if block.at not in reachable:
                continue
            result = through(block, entering(block.at))
            if result != leaving[block.at]:
                leaving[block.at] = result
                changed = True
        if not changed:
            break
    reads = set()
    for block in body.blocks:
        if block.at in reachable:
            through(block, entering(block.at), reads)
    return reads


def promoted(
    body: MirBody,
    dgroup: frozenset[int] = frozenset(),
    bounds: dict | None = None,
    *,
    loop_only: bool = False,
    split_updates: bool = True,
) -> MirBody:
    """Reuse eligible stored values without removing observable writes."""
    original = body
    body = _separated(body) if split_updates else body
    found = promotable(body, dgroup, bounds)
    if loop_only:
        hot = {at for loop in loops.loops(body.blocks, body.entry) for at in loop.body}
        read = {_key(ref) for block in body.blocks if block.at in hot for op in block.ops for ref in op.loads}
        found = {addr: width for addr, width in found.items() if addr in read}
    if not found:
        return original
    usable = _available(body, found, dgroup, bounds)
    updates = {op.id for block in original.blocks for op in block.ops if op.loads and op.stores}
    if split_updates and any(
        op.id in updates and op.loads and not op.stores and id(op) not in usable
        for block in body.blocks
        for op in block.ops
    ):
        return promoted(original, dgroup, bounds, loop_only=loop_only, split_updates=False)

    taken = max((one.variable for one in ssa.values(body)), default=0)
    fresh = _next(body)
    holds = {}
    for number, addr in enumerate(sorted(found, key=_order), 1):
        holds[addr] = taken + number

    changed = False
    initializers = _initializers(body, found, dgroup, bounds)
    blocks = []
    for block in body.blocks:
        ops = []
        for op in block.ops:
            initialized = initializers.get(id(op), {})
            if initialized:
                ops.append(op)
                exact = _instead(op, holds, found, fresh)
                if exact is not None:
                    fresh += 1
                    ops.append(replace(exact, node=None, id=None, covers=(op.at, op.at), extra_covers=(), symbol=False))
                for addr, fact in initialized.items():
                    value = mir.Value(fresh, op.at, variable=holds[addr], version=1)
                    fresh += 1
                    ops.append(
                        replace(
                            op,
                            kind=mir.Kind.COPY,
                            name="mov",
                            defines=(value,),
                            uses=(),
                            loads=(),
                            stores=(),
                            merges={},
                            args=(mir.Const(fact.n, fact.width),),
                            results=(mir.Held(value, fact.width),),
                            node=None,
                            raised=None,
                            id=None,
                            covers=(op.at, op.at),
                            extra_covers=(),
                            symbol=False,
                        )
                    )
                changed = True
                continue
            if op.loads and id(op) not in usable:
                ops.append(op)
                continue
            made = _instead(op, holds, found, fresh)
            if made is None:
                ops.append(op)
                continue
            fresh += 1
            changed = True
            if op.stores:
                ops.append(op)
                made = replace(made, node=None, id=None, covers=(op.at, op.at), symbol=False)
            ops.append(made)
        blocks.append(replace(block, ops=tuple(ops)))
    if not changed:
        return body
    return ssa.constructed(replace(body, blocks=tuple(blocks)), frozenset(holds.values()))


def _separated(body: MirBody) -> MirBody:
    """Expose a memory update as a value computation and an observable store."""
    fresh = _next(body)
    variable = max((one.variable for one in ssa.values(body)), default=0) + 1
    blocks = []
    for block in body.blocks:
        ops = []
        for op in block.ops:
            if (
                op.kind not in READS - {mir.Kind.LOAD}
                or len(op.loads) != 1
                or op.loads != op.stores
                or op.results != (mir.Cell(op.loads[0]),)
                or any(not value.flags for value in op.defines)
                or op.loads[0].base is not None
                or op.loads[0].segment is not None
            ):
                ops.append(op)
                continue
            result = mir.Value(fresh, op.at, variable=variable, version=1)
            fresh += 1
            variable += 1
            held = mir.Held(result, op.loads[0].width)
            ops.append(replace(op, results=(held,), stores=(), defines=(result, *op.defines), symbol=False))
            ops.append(
                replace(
                    op,
                    kind=mir.Kind.STORE,
                    name="mov",
                    args=(held,),
                    loads=(),
                    defines=(),
                    uses=(result,),
                    node=None,
                    raised=None,
                    covers=(op.at, op.at),
                    extra_covers=(),
                    merges={},
                    symbol=True,
                )
            )
        blocks.append(replace(block, ops=tuple(ops)))
    return replace(body, blocks=tuple(blocks))


def _instead(op: Op, holds: dict, found: dict, fresh: int) -> "Op | None":
    """The value operation paired with a store, or replacing a memory read.

    A store becomes a definition of that variable and a load a use of it.
    Only where the cell is the operation's whole memory traffic: one that
    also touches something else is left alone rather than half rewritten,
    and a half-rewritten body reads stale memory.
    """
    cell = _cell(op)
    addr = _key(cell) if cell is not None else None
    if cell is None or addr not in holds or cell.width != found[addr]:
        return None
    width = found[addr]
    variable = holds[addr]

    if op.stores and not op.loads:
        # `mov [x],ax` is `x := ax`, and x is a variable now.
        into = mir.Value(id=fresh, at=op.at, variable=variable, version=1)
        return replace(
            op,
            kind=mir.Kind.COPY,
            name="mov",
            defines=(into, *(one for one in op.defines if one.flags)),
            stores=(),
            results=(mir.Held(into, width),),
            args=tuple(one for one in op.args if not isinstance(one, mir.Cell)),
            uses=tuple(one.value for one in op.args if isinstance(one, mir.Held)),
        )

    if op.loads:
        # `mov ax,[x]` is `ax := x`. The version is a placeholder --
        # `mir.resolved()` renames per variable and settles which one.
        holding = mir.Value(id=fresh, at=op.at, variable=variable, version=1)
        return replace(
            op,
            kind=mir.Kind.COPY if op.kind is mir.Kind.LOAD else op.kind,
            uses=tuple(
                dict.fromkeys(
                    [one.value for one in op.args if isinstance(one, mir.Held)]
                    + [one for one in op.uses if one.flags]
                    + [holding]
                )
            ),
            loads=(),
            args=tuple(mir.Held(holding, width) if isinstance(one, mir.Cell) else one for one in op.args),
        )
    return None


def _next(body: MirBody) -> int:
    """An id nothing in this body uses."""
    return max((one.id for one in ssa.values(body)), default=0) + 1
