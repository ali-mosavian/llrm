"""Write-through promotion: reuse a stored value across statements.

Inspired by LLVM's `mem2reg`. A variable BC keeps in memory and reloads for every
statement -- because it compiles a statement at a time, which is the fact
under every number in `docs/targets.md` -- becomes an SSA value, and the
register allocator can keep that value across statements. Unlike LLVM's
local allocas, these cells can be visible outside this body: every store
stays in place. Removing stores needs a separate proof of observability.

**Which cells.** A fixed address in the program's own data, at one width.
A read reuses the stored value only when it is available along every
incoming path. Any possibly aliasing write invalidates that availability;
a later direct store establishes it again. Runtime calls use the same
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
from collections.abc import Iterator

from qbopt import mir
from qbopt import loops
from qbopt.mir import Op
from qbopt.mir import MirBody
from qbopt.module import Space
from qbopt.passes import Where
from qbopt.passes import MIRTransform

READS = frozenset(
    {
        mir.Kind.LOAD,
        mir.Kind.ADD,
        mir.Kind.SUB,
        mir.Kind.MUL,
        mir.Kind.AND,
        mir.Kind.OR,
        mir.Kind.XOR,
    }
)


class Promote(MIRTransform):
    name = "promote"

    def __init__(self, where: Where) -> None:
        self.where = where

    def transform(self, body: MirBody) -> MirBody:
        return promoted(body, self.where.dgroup, self.where.bounds, loop_only=True)


def promotable(body: MirBody, dgroup: frozenset[int] = frozenset(), bounds: dict | None = None) -> dict:
    """Cells whose reads can use a known stored value, by address.

    Touched more than once, because promoting a cell read or written once
    removes no work. At one width, because a long stored as two words and
    read as one is two variables in the same place and this does not model
    that. Availability excludes reads after intervening aliasing writes.
    """
    every = [one for block in body.blocks for op in block.ops for one in (*op.loads, *op.stores)]
    read = {ref.addr for block in body.blocks for op in block.ops for ref in op.loads}
    seen: Counter = Counter()
    widths: dict = {}
    for one in every:
        if one.addr is None or one.base is not None or one.addr.space is not Space.SEGMENT:
            continue
        seen[one.addr] += 1
        widths.setdefault(one.addr, set()).add(one.width)

    candidates = {
        addr: next(iter(widths[addr]))
        for addr, times in seen.items()
        if times > 1 and len(widths[addr]) == 1 and addr in read
    }
    usable = _available(body, candidates, dgroup, bounds)
    used = {ref.addr for block in body.blocks for op in block.ops if id(op) in usable for ref in op.loads}
    return {addr: width for addr, width in candidates.items() if addr in used}


def _cell(op: Op) -> mir.MemRef | None:
    """The whole access this pass can replace, never part of an operation."""
    match op.kind, op.loads, op.stores:
        case kind, (ref,), () if kind in READS:
            return ref if ref.base is None else None
        case mir.Kind.STORE, (), (ref,):
            return ref if ref.base is None else None
    return None


def _available(body: MirBody, cells: dict, dgroup: frozenset[int], bounds: dict | None) -> set[int]:
    """Reads reached by a stored value on every path, without an intervening aliasing write."""
    reachable = {at for at, doms in loops.dominators(body.blocks, body.entry).items() if doms}
    predecessors = loops.predecessors(body.blocks)
    leaving = {at: set(cells) for at in reachable}
    refs = {addr: mir.MemRef(addr, width) for addr, width in cells.items()}

    def entering(at: int) -> set:
        parents = predecessors[at] & reachable
        return set.intersection(*(leaving[parent] for parent in parents)) if parents and at != body.entry else set()

    def through(block: mir.MirBlock, available: set, reads: set[int] | None = None) -> set:
        for op in block.ops:
            cell = _cell(op)
            if reads is not None and op.loads and cell is not None and cell.addr in available:
                reads.add(id(op))
            available.difference_update(
                addr
                for addr, ref in refs.items()
                if any(mir.overlapping(ref, written, dgroup, bounds) for written in op.stores)
            )
            if op.kind is mir.Kind.STORE and cell is not None and cell.addr in cells:
                available.add(cell.addr)
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
    body: MirBody, dgroup: frozenset[int] = frozenset(), bounds: dict | None = None, *, loop_only: bool = False
) -> MirBody:
    """Reuse eligible stored values without removing observable writes."""
    found = promotable(body, dgroup, bounds)
    if loop_only:
        hot = {at for loop in loops.loops(body.blocks, body.entry) for at in loop.body}
        read = {ref.addr for block in body.blocks if block.at in hot for op in block.ops for ref in op.loads}
        found = {addr: width for addr, width in found.items() if addr in read}
    if not found:
        return body
    usable = _available(body, found, dgroup, bounds)

    values = tuple(_values(body))
    taken = max((one.variable for one in values), default=0)
    fresh = _next(body)
    holds = {}
    for number, addr in enumerate(sorted(found, key=lambda one: (one.index, one.disp)), 1):
        holds[addr] = taken + number

    changed = False
    blocks = []
    for block in body.blocks:
        ops = []
        for op in block.ops:
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
                made = replace(made, node=None, made=None, id=None, covers=(op.at, op.at), symbol=False)
            ops.append(made)
        blocks.append(replace(block, ops=tuple(ops)))
    if not changed:
        return body
    return _ssa(replace(body, blocks=tuple(blocks)), frozenset(holds.values()))


def _ssa(body: MirBody, variables: frozenset[int]) -> MirBody:
    """Place SSA only for new memory variables; existing SSA edges are authoritative."""

    def owned(values: tuple[mir.Value, ...]) -> tuple[mir.Value, ...]:
        return tuple(one for one in values if one.variable in variables)

    skeleton = replace(
        body,
        origin={},
        pins={},
        blocks=tuple(
            replace(
                block,
                phis=(),
                ops=tuple(
                    replace(
                        op,
                        defines=owned(op.defines),
                        uses=owned(op.uses),
                        args=(),
                        results=(),
                        loads=(),
                        stores=(),
                        merges={},
                        raised=None,
                    )
                    for op in block.ops
                ),
            )
            for block in body.blocks
        ),
    )
    repaired = mir.resolved(skeleton)
    if isinstance(repaired, str):
        raise mir.Unraisable(repaired)

    def merge(op: Op, fixed: Op) -> Op:
        uses = {one.variable: one for one in fixed.uses}
        defines = {one.variable: one for one in fixed.defines}

        def operand(one: mir.Arg, names: dict[int, mir.Value]) -> mir.Arg:
            return (
                mir.Held(names[one.value.variable], one.width)
                if (isinstance(one, mir.Held) and one.value.variable in names)
                else one
            )

        return replace(
            op,
            uses=tuple(uses.get(one.variable, one) for one in op.uses),
            defines=tuple(defines.get(one.variable, one) for one in op.defines),
            args=tuple(operand(one, uses) for one in op.args),
            results=tuple(operand(one, defines) for one in op.results),
        )

    # The isolated renamer starts ids at zero; keep its namespace disjoint.
    offset = _next(body)

    def shifted(value: mir.Value) -> mir.Value:
        return replace(value, id=value.id + offset)

    mapping = {one: shifted(one) for one in _values(repaired)}
    repaired = replace(
        repaired,
        blocks=tuple(
            replace(
                block,
                phis=tuple(
                    mir.Phi(mapping[phi.result], {at: mapping[value] for at, value in phi.incoming.items()})
                    for phi in block.phis
                ),
                ops=tuple(
                    replace(
                        op,
                        uses=tuple(mapping[one] for one in op.uses),
                        defines=tuple(mapping[one] for one in op.defines),
                    )
                    for op in block.ops
                ),
            )
            for block in repaired.blocks
        ),
    )
    return replace(
        body,
        blocks=tuple(
            replace(
                block,
                phis=block.phis + fixed.phis,
                ops=tuple(merge(op, changed) for op, changed in zip(block.ops, fixed.ops, strict=True)),
            )
            for block, fixed in zip(body.blocks, repaired.blocks, strict=True)
        ),
    )


def _instead(op: Op, holds: dict, found: dict, fresh: int) -> "Op | None":
    """The value operation paired with a store, or replacing a memory read.

    A store becomes a definition of that variable and a load a use of it.
    Only where the cell is the operation's whole memory traffic: one that
    also touches something else is left alone rather than half rewritten,
    and a half-rewritten body reads stale memory.
    """
    cell = _cell(op)
    if cell is None or cell.addr not in holds:
        return None
    addr = cell.addr
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
        )

    if op.loads:
        # `mov ax,[x]` is `ax := x`. The version is a placeholder --
        # `mir.resolved()` renames per variable and settles which one.
        holding = mir.Value(id=fresh, at=op.at, variable=variable, version=1)
        return replace(
            op,
            kind=mir.Kind.COPY if op.kind is mir.Kind.LOAD else op.kind,
            uses=op.uses + (holding,),
            loads=(),
            args=tuple(mir.Held(holding, width) if isinstance(one, mir.Cell) else one for one in op.args),
        )
    return None


def _next(body: MirBody) -> int:
    """An id nothing in this body uses."""
    return max((one.id for one in _values(body)), default=0) + 1


def _values(body: MirBody) -> Iterator[mir.Value]:
    for block in body.blocks:
        for op in block.ops:
            yield from op.defines
            yield from op.uses
        for phi in block.phis:
            yield phi.result
            yield from phi.incoming.values()
