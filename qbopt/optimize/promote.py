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
from dataclasses import dataclass
from dataclasses import replace
from math import prod

from qbopt.model import mir
from qbopt.model import memory
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


@dataclass(frozen=True, slots=True)
class _Leaf:
    """One exact scalar leaf of a canonical memory object.

    Address SSA values describe how an operation reaches the leaf, not what
    the leaf is.  Provenance is authoritative only when it names one dense,
    exact byte range matching the access width; wider, strided and union
    provenance stays as ordinary memory.
    """

    object: memory.Object
    low: int
    high: int
    type_class: str | None


@dataclass(frozen=True, slots=True)
class _Affine:
    """One symbolic address root plus a mathematical byte displacement."""

    root: object | None
    offset: int


def _leaf(ref: mir.MemRef) -> _Leaf | None:
    provenance = ref.provenance
    if provenance is None or len(provenance.slices) != 1:
        return None
    span = next(iter(provenance.slices))
    # Frontends spell one contiguous exact access either as the canonical
    # byte range ``[low, low + width)`` or as one stride-1 element whose own
    # width is the access width.  They select exactly the same bytes; keep
    # that representational difference out of SROA's object identity.
    contiguous = (
        (span.width == 1 and span.high - span.low == ref.width)
        or (span.high - span.low == 1 and span.width == ref.width)
    )
    if span.stride != 1 or not contiguous:
        return None
    high = span.low + ref.width
    if span.object.extent is not None and not (0 <= span.low < high <= span.object.extent):
        return None
    return _Leaf(span.object, span.low, high, None if ref.typed is None else ref.typed[0])


def _blocked(refs: list[mir.MemRef]) -> frozenset[memory.Slice]:
    """Bytes whose accesses cannot form disjoint scalar leaves.

    Equal ranges are repeated accesses to one leaf.  Disjoint ranges are
    independent leaves.  A proper overlap means that no leaf partition can
    represent both accesses without observable partial writes, so both stay
    memory; the rest of their object is unaffected.  Ambiguous multi-object
    provenance is likewise not an identity on which scalar replacement may be
    based, so every slice it names stays memory.
    """
    accesses: dict[memory.Object, list[_Leaf]] = {}
    blocked: set[memory.Slice] = set()
    for ref in refs:
        provenance = ref.provenance
        if provenance is None:
            continue
        if len({span.object for span in provenance.slices}) != 1 or len(provenance.slices) != 1:
            blocked.update(provenance.slices)
            continue
        if (leaf := _leaf(ref)) is not None:
            accesses.setdefault(leaf.object, []).append(leaf)

    for object_, leaves in accesses.items():
        for index, one in enumerate(leaves):
            for other in leaves[index + 1 :]:
                overlaps = max(one.low, other.low) < min(one.high, other.high)
                same_range = (one.low, one.high) == (other.low, other.high)
                if overlaps and (not same_range or one.type_class != other.type_class):
                    blocked.add(memory.Slice(object_, one.low, one.high))
                    blocked.add(memory.Slice(object_, other.low, other.high))
    return frozenset(blocked)


def _touches(ref: mir.MemRef, blocked: frozenset[memory.Slice]) -> bool:
    """Whether the reference reaches a byte of its own object that is blocked."""
    spans = () if ref.provenance is None else ref.provenance.slices
    return any(span.object == one.object and span.intersects(one) for span in spans for one in blocked)


def _key(ref, blocked: frozenset[memory.Slice] = frozenset()):
    if ref.volatile:
        return None
    if _touches(ref, blocked):
        return None
    if (leaf := _leaf(ref)) is not None:
        return leaf
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
    if isinstance(key, _Leaf):
        provenance = memory.Provenance.one(key.object, key.low, key.high)
        return mir.MemRef(None, width, provenance=provenance)
    return replace(key, width=width) if isinstance(key, mir.MemRef) else mir.MemRef(key, width)


def _order(key):
    if isinstance(key, _Leaf):
        return (
            1,
            str(key.object.kind),
            repr(key.object.identity),
            key.object.generation,
            key.low,
            key.high,
            "" if key.type_class is None else key.type_class,
        )
    ref = _reference(key, 0)
    return 0, ref.addr.index, ref.addr.disp, -1 if ref.base is None else ref.base.id


def _aggregate_objects(leaves) -> frozenset[memory.Object]:
    """Objects known to contain more than the scalar leaf being accessed."""
    ranges: dict[memory.Object, set[tuple[int, int]]] = {}
    for leaf in leaves:
        if isinstance(leaf, _Leaf):
            ranges.setdefault(leaf.object, set()).add((leaf.low, leaf.high))
    return frozenset(
        object_
        for object_, parts in ranges.items()
        if len(parts) > 1
        or (
            object_.extent is not None
            and any(object_.extent > high - low for low, high in parts)
        )
    )


class Promote(MIRTransform):
    name = "promote"

    def __init__(self, where: Where) -> None:
        self.where = where

    def transform(self, body: MirBody) -> MirBody:
        return promoted(body, self.where.dgroup, self.where.bounds)


class Sroa(MIRTransform):
    """Scalarize proven aggregate leaves before scalar simplification."""

    name = "sroa"

    def __init__(self, where: Where) -> None:
        self.where = where

    def transform(self, body: MirBody) -> MirBody:
        body = _allocation_leaves(body)
        body = _bounded_leaves(body)
        body = _canonical_leaf_types(body)
        body = _split_copies(body)
        return promoted(body, self.where.dgroup, self.where.bounds, aggregate_only=True)


def _signed(number: int, width: int) -> int:
    sign = 1 << (width * 8 - 1)
    return ((number & ((sign << 1) - 1)) ^ sign) - sign


def _affine_values(body: MirBody) -> dict[mir.Value, _Affine]:
    """Normalize address arithmetic without giving physical locations meaning.

    A direct cell is an opaque symbolic root.  Addition, subtraction and
    width conversion retain that root and combine only byte displacements.
    This is deliberately not general symbolic algebra: two roots, products,
    phis and unknown operations are refused.
    """
    definitions = {
        result.value: op
        for block in body.blocks
        for op in block.ops
        for result in op.results
        if isinstance(result, mir.Held)
    }
    cache: dict[mir.Value, _Affine | None] = {}
    active: set[mir.Value] = set()

    def operand(arg: mir.Arg) -> _Affine | None:
        if isinstance(arg, mir.Const):
            return _Affine(None, _signed(arg.n, arg.width))
        if isinstance(arg, mir.Held):
            return value(arg.value)
        if not isinstance(arg, mir.Cell) or arg.ref.volatile:
            return None
        ref = mir._symbolic_ref(arg.ref)
        if ref.addr is None or ref.base is not None or ref.segment is not None:
            return None
        return _Affine(("cell", ref.addr, ref.width, ref.space), 0)

    def combined(kind: mir.Kind, left: _Affine, right: _Affine) -> _Affine | None:
        if kind is mir.Kind.ADD:
            if left.root is not None and right.root is not None:
                return None
            return _Affine(left.root if left.root is not None else right.root, left.offset + right.offset)
        if kind is mir.Kind.SUB and right.root is None:
            return _Affine(left.root, left.offset - right.offset)
        return None

    def value(one: mir.Value) -> _Affine | None:
        if one in cache:
            return cache[one]
        if one in active:
            return None
        active.add(one)
        op = definitions.get(one)
        result = None
        if op is not None and not op.barrier and not op.volatile:
            args = tuple(operand(arg) for arg in op.args)
            if all(arg is not None for arg in args):
                known = tuple(arg for arg in args if arg is not None)
                if op.kind in (mir.Kind.COPY, mir.Kind.LOAD, mir.Kind.ZERO_EXTEND):
                    result = known[0] if len(known) == 1 else None
                elif op.kind in (mir.Kind.ADD, mir.Kind.SUB) and len(known) == 2:
                    result = combined(op.kind, known[0], known[1])
                elif op.kind is mir.Kind.INCREMENT and len(known) == 1:
                    result = replace(known[0], offset=known[0].offset + 1)
                elif op.kind is mir.Kind.DECREMENT and len(known) == 1:
                    result = replace(known[0], offset=known[0].offset - 1)
        active.remove(one)
        cache[one] = result
        return result

    return {one: fact for one in definitions if (fact := value(one)) is not None}


def _allocation_leaves(body: MirBody) -> MirBody:
    """Attach exact relative leaves to affine accesses of one allocation.

    ``MemRef.allocation`` already proves that an access is inside the current
    owning allocation.  Frontends may nevertheless spell its physical offset
    with different SSA expressions and widths.  Group expressions by their
    opaque root, normalize their constant differences, and make those byte
    ranges ordinary canonical memory provenance.  The minimum observed offset
    is only a coordinate origin; no assumption is made about the heap address.
    """
    requests: dict[mir.Symbol, list[tuple[int, int]]] = {}
    for block in body.blocks:
        for op in block.ops:
            request = op.array
            if request is None or request.replaces or request.element_width <= 0:
                continue
            count = prod(high - low + 1 for low, high in request.bounds)
            extent = request.element_width * count
            if count > 0 and 0 < extent < 1 << 31:
                requests.setdefault(request.descriptor, []).append((op.id if op.id is not None else op.at, extent))
    unique = {descriptor: found[0] for descriptor, found in requests.items() if len(found) == 1}
    if not unique:
        return body

    affine = _affine_values(body)
    grouped: dict[tuple[mir.Symbol, int, int, object], list[tuple[mir.MemRef, int]]] = {}
    for block in body.blocks:
        for op in block.ops:
            for ref in (*op.loads, *op.stores):
                request = unique.get(ref.allocation)
                fact = affine.get(ref.base) if ref.base is not None else None
                if request is None or fact is None or fact.root is None or ref.addr is None or ref.width <= 0:
                    continue
                generation, extent = request
                grouped.setdefault((ref.allocation, generation, extent, fact.root), []).append(
                    (ref, fact.offset + ref.addr.disp)
                )

    exact: dict[mir.MemRef, memory.Provenance] = {}
    for (descriptor, generation, extent, root), accesses in grouped.items():
        origin = min(offset for _ref, offset in accesses)
        if any(not 0 <= offset - origin <= extent - ref.width for ref, offset in accesses):
            continue
        object_ = memory.Object(memory.Kind.ALLOCATION, (descriptor, generation, root), extent=extent)
        for ref, offset in accesses:
            low = offset - origin
            exact[ref] = memory.Provenance.one(object_, low, low + ref.width)
    if not exact:
        return body

    def reference(ref: mir.MemRef) -> mir.MemRef:
        provenance = exact.get(ref)
        return ref if provenance is None else replace(ref, provenance=provenance)

    def operand(arg: mir.Arg) -> mir.Arg:
        return mir.Cell(reference(arg.ref)) if isinstance(arg, mir.Cell) else arg

    return replace(
        body,
        blocks=tuple(
            replace(
                block,
                ops=tuple(
                    replace(
                        op,
                        loads=tuple(map(reference, op.loads)),
                        stores=tuple(map(reference, op.stores)),
                        args=tuple(map(operand, op.args)),
                        results=tuple(map(operand, op.results)),
                        memory_values=tuple((reference(ref), value) for ref, value in op.memory_values),
                    )
                    for op in block.ops
                ),
            )
            for block in body.blocks
        ),
    )


def _bounded_ref(ref: mir.MemRef, known: dict) -> mir.MemRef:
    """Give one singleton indexed access its exact object slice.

    Whole-object provenance establishes identity; the range establishes the
    byte offset.  Both are required, and the object's extent proves the
    addition cannot select or wrap outside that object.
    """
    if ref.base is None or ref.addr is None or ref.provenance is None or len(ref.provenance.slices) != 1:
        return ref
    interval = known.get(ref.base)
    if interval is None or interval.width != ref.base_width or interval.low != interval.high:
        return ref
    source = next(iter(ref.provenance.slices))
    extent = source.object.extent
    if extent is None:
        return ref
    # Only whole-object provenance can be narrowed this way.  A pointer into
    # a subobject already has an offset origin of its own.
    if source.low > 0 or source.high < extent:
        return ref
    low = ref.addr.disp + interval.low
    high = low + ref.width
    if not 0 <= low < high <= extent:
        return ref
    provenance = memory.Provenance(
        frozenset({memory.Slice(source.object, low, high)}),
        ref.provenance.restrict,
    )
    return replace(ref, provenance=provenance)


def _pointed_ref(
    ref: mir.MemRef,
    pointers: dict[mir.Value, memory.Provenance],
) -> mir.MemRef:
    """Give an exact object-relative pointer access its scalar leaf.

    A pointer value is an address, not an integer range.  The alias analysis
    already follows frame addresses, copies and constant byte offsets while
    retaining canonical object identity.  Use that fact only when it denotes
    one exact byte position inside the same bounded object named by the
    reference's whole-object provenance.
    """
    if ref.base is None or ref.provenance is None or len(ref.provenance.slices) != 1:
        return ref
    source = next(iter(ref.provenance.slices))
    provenance = pointers.get(ref.base)
    if provenance is None or len(provenance.slices) != 1:
        return ref
    address = next(iter(provenance.slices))
    if address.object != source.object or address.stride != 1 or address.width != 1 or address.high - address.low != 1:
        return ref
    low = address.low + (0 if ref.addr is None else ref.addr.disp)
    high = low + ref.width
    extent = address.object.extent
    if extent is None or not 0 <= low < high <= extent:
        return ref
    # The frontend annotation may be the conservative lane selected by the
    # original indexed expression rather than the whole aggregate.  An exact
    # same-object pointer fact is allowed to refine it only when that lane
    # really covers every byte of the candidate access.  Requiring a whole
    # object here made exact cloned accesses depend on which broad spelling
    # their frontend happened to retain after unrolling.
    def covered(byte: int) -> bool:
        return any(
            source.low <= byte - lane < source.high and (byte - lane - source.low) % source.stride == 0
            for lane in range(source.width)
        )

    if not all(covered(byte) for byte in range(low, high)):
        return ref
    exact = memory.Provenance(
        frozenset({memory.Slice(source.object, low, high)}),
        ref.provenance.restrict,
    )
    return replace(ref, provenance=exact)


def _bounded_leaves(body: MirBody) -> MirBody:
    """Materialize exact singleton proofs on every indexed-ref occurrence.

    General loop intervals select several elements and therefore cannot be
    one scalar leaf.  Constant propagation already computes every singleton
    expression to a fixed point; running the much heavier loop-range analysis
    here more than doubled compile time and produced no additional leaf.
    """
    from qbopt.analysis import alias
    from qbopt.analysis import ranges

    constants = ranges.singletons(body)
    pointers = alias.points_to(body).values
    blocks = []
    for block in body.blocks:
        def reference(ref: mir.MemRef) -> mir.MemRef:
            return _pointed_ref(_bounded_ref(ref, constants), pointers)

        def operand(arg: mir.Arg) -> mir.Arg:
            return mir.Cell(reference(arg.ref)) if isinstance(arg, mir.Cell) else arg

        ops = tuple(
            replace(
                op,
                loads=tuple(map(reference, op.loads)),
                stores=tuple(map(reference, op.stores)),
                args=tuple(map(operand, op.args)),
                results=tuple(map(operand, op.results)),
                memory_values=tuple((reference(ref), value) for ref, value in op.memory_values),
            )
            for op in block.ops
        )
        blocks.append(replace(block, ops=ops))
    return replace(body, blocks=tuple(blocks))


def _canonical_leaf_types(body: MirBody) -> MirBody:
    """Attach an established scalar type to an otherwise untyped same-size leaf.

    A C aggregate move is byte-typed at the raise, while a later field access
    carries the field's scalar type.  They are the same exact bytes, not a
    type-punning access: the former simply has no more precise source type to
    report.  Canonicalizing only that ``None`` spelling lets the scalar leaf
    represent both occurrences.  Two explicit, distinct type classes retain
    the existing conservative union/type-pun rejection.
    """
    types: dict[tuple[memory.Object, int, int], set[str]] = {}
    for block in body.blocks:
        for op in block.ops:
            if op.source_backed or op.absorbed or op.id is None:
                continue
            for ref in (*op.loads, *op.stores):
                if (leaf := _leaf(ref)) is not None and leaf.type_class is not None:
                    types.setdefault((leaf.object, leaf.low, leaf.high), set()).add(leaf.type_class)

    known = {key: next(iter(classes)) for key, classes in types.items() if len(classes) == 1}
    if not known:
        return body

    def reference(ref: mir.MemRef) -> mir.MemRef:
        leaf = _leaf(ref)
        if leaf is None or ref.typed is not None:
            return ref
        type_class = known.get((leaf.object, leaf.low, leaf.high))
        return ref if type_class is None else replace(ref, typed=(type_class, False))

    def operand(arg: mir.Arg) -> mir.Arg:
        return mir.Cell(reference(arg.ref)) if isinstance(arg, mir.Cell) else arg

    return replace(
        body,
        blocks=tuple(
            replace(
                block,
                ops=tuple(
                    op
                    if op.source_backed or op.absorbed or op.id is None
                    else replace(
                        op,
                        loads=tuple(map(reference, op.loads)),
                        stores=tuple(map(reference, op.stores)),
                        args=tuple(map(operand, op.args)),
                        results=tuple(map(operand, op.results)),
                        memory_values=tuple((reference(ref), value) for ref, value in op.memory_values),
                    )
                    for op in block.ops
                ),
            )
            for block in body.blocks
        ),
    )


def _copy_partition(ref: mir.MemRef, leaves: tuple[_Leaf, ...]) -> tuple[_Leaf, ...] | None:
    """The exact scalar partition of a whole-object move destination, if any."""
    whole = _leaf(ref)
    if whole is None:
        return None
    pieces = tuple(
        sorted(
            {
                leaf
                for leaf in leaves
                if leaf.object == whole.object
                and whole.low <= leaf.low < leaf.high <= whole.high
                and (leaf.low, leaf.high) != (whole.low, whole.high)
            },
            key=lambda leaf: (leaf.low, leaf.high),
        )
    )
    if len(pieces) < 2:
        return None
    at = whole.low
    for piece in pieces:
        if piece.low != at:
            return None
        at = piece.high
    return pieces if at == whole.high else None


def _copy_piece(ref: mir.MemRef, whole: _Leaf, piece: _Leaf, *, low: int) -> mir.MemRef:
    """One direct scalar byte range of a proven whole-object access."""
    assert ref.addr is not None
    return replace(
        ref,
        addr=ref.addr.plus(low - whole.low),
        width=piece.high - piece.low,
        typed=None if piece.type_class is None else (piece.type_class, False),
        provenance=memory.Provenance.one(whole.object, low, low + piece.high - piece.low),
    )


def _split_copies(body: MirBody) -> MirBody:
    """Expand a proven exact aggregate copy into its existing scalar leaves.

    This is deliberately stricter than an ordinary copy: both sides must be
    exact references, their objects must be known disjoint, and the
    destination's complete byte range must already have a contiguous scalar
    partition. A destination may use one proven near pointer, but its base
    remains an explicit use on every generated store.
    Far, indexed, volatile, overlapping, or incompletely partitioned copies
    stay whole: changing one uncertain wide access into several accesses
    could alter fault or tearing behavior. The C aggregate move has no
    source-byte owner; source-backed object instructions stay untouched for
    the source-map emitter.
    """
    leaves = tuple(
        leaf
        for block in body.blocks
        for op in block.ops
        for ref in (*op.loads, *op.stores)
        if (leaf := _leaf(ref)) is not None
    )
    fresh = _next(body)
    variable = max((one.variable for one in ssa.values(body)), default=0) + 1
    changed = False
    blocks = []

    for block in body.blocks:
        ops: list[Op] = []
        index = 0
        while index < len(block.ops):
            load = block.ops[index]
            store = block.ops[index + 1] if index + 1 < len(block.ops) else None
            candidate = _copy_candidate(load, store, leaves)
            if candidate is None:
                ops.append(load)
                index += 1
                continue
            source, destination, source_leaf, destination_leaf, pieces = candidate
            for piece in pieces:
                source_piece = _copy_piece(
                    source,
                    source_leaf,
                    piece,
                    low=source_leaf.low + piece.low - destination_leaf.low,
                )
                destination_piece = _copy_piece(destination, destination_leaf, piece, low=piece.low)
                width = piece.high - piece.low
                value = mir.Value(fresh, load.at, variable=variable, version=1)
                fresh += 1
                variable += 1
                held = mir.Held(value, width)
                ops.append(
                    replace(
                        load,
                        defines=(value,),
                        uses=tuple(one for one in (source_piece.base, source_piece.segment) if one is not None),
                        loads=(source_piece,),
                        stores=(),
                        args=(mir.Cell(source_piece),),
                        results=(held,),
                        source_backed=False,
                        raised=None,
                        id=None,
                        absorbed=(),
                        symbol=None,
                    )
                )
                ops.append(
                    replace(
                        store,
                        defines=(),
                        uses=tuple(
                            one for one in (value, destination_piece.base, destination_piece.segment) if one is not None
                        ),
                        loads=(),
                        stores=(destination_piece,),
                        args=(held,),
                        results=(mir.Cell(destination_piece),),
                        source_backed=False,
                        raised=None,
                        id=None,
                        absorbed=(),
                        symbol=None,
                    )
                )
            changed = True
            index += 2
        blocks.append(replace(block, ops=tuple(ops)))
    return replace(body, blocks=tuple(blocks)) if changed else body


def _copy_candidate(load: Op, store: Op | None, leaves: tuple[_Leaf, ...]):
    """Return the proof for one adjacent exact C aggregate copy, otherwise ``None``."""
    if (
        store is None
        or load.source_backed
        or store.source_backed
        or load.absorbed
        or store.absorbed
        or load.id is None
        or store.id is None
        or load.volatile
        or store.volatile
    ):
        return None
    if (
        load.kind is not mir.Kind.LOAD
        or store.kind is not mir.Kind.STORE
        or len(load.loads) != 1
        or load.stores
        or len(store.stores) != 1
        or store.loads
        or len(load.results) != 1
        or len(store.args) != 1
        or not isinstance(load.results[0], mir.Held)
        or not isinstance(store.args[0], mir.Held)
        or load.results[0].value != store.args[0].value
        or load.defines != (load.results[0].value,)
        or store.defines
    ):
        return None
    source, destination = load.loads[0], store.stores[0]
    source_leaf, destination_leaf = _leaf(source), _leaf(destination)
    if (
        source_leaf is None
        or destination_leaf is None
        or source.width != destination.width
        or source.volatile
        or destination.volatile
        or source.pointer
        or destination.pointer
        or destination.segment is not None
        or source.addr is None
        or destination.addr is None
        or not source.addr.direct
        or not destination.addr.direct
        or memory.objects_may_alias(source_leaf.object, destination_leaf.object)
    ):
        return None
    source_inputs = tuple(one for one in (source.base, source.segment) if one is not None)
    destination_inputs = tuple(one for one in (destination.base, destination.segment) if one is not None)
    if load.uses != source_inputs or store.uses != (store.args[0].value, *destination_inputs):
        return None
    pieces = _copy_partition(destination, leaves)
    if pieces is None:
        return None
    return source, destination, source_leaf, destination_leaf, pieces


def promotable(
    body: MirBody,
    dgroup: frozenset[int] = frozenset(),
    bounds: dict | None = None,
    *,
    aggregate_only: bool = False,
) -> dict:
    """Cells whose reads can use a known stored value, by address.

    Touched more than once, because promoting a cell read or written once
    removes no work. Supported reads must agree on one width. Exact stores and
    complete constant initializers can establish that value; arbitrary partial
    writes cannot. Availability excludes reads after intervening aliasing writes.
    """
    every = [one for block in body.blocks for op in block.ops for one in (*op.loads, *op.stores)]
    blocked = _blocked(every)
    seen: Counter = Counter()
    widths: dict = {}
    for one in every:
        key = _key(one, blocked)
        if key is None:
            continue
        seen[key] += 1
    for block in body.blocks:
        for op in block.ops:
            if op.loads and (ref := _cell(op)) is not None:
                widths.setdefault(_key(ref, blocked), set()).add(ref.width)

    candidates = {
        addr: next(iter(widths[addr]))
        for addr, times in seen.items()
        if times > 1 and addr in widths and len(widths[addr]) == 1
    }
    if aggregate_only:
        aggregates = _aggregate_objects(candidates)
        candidates = {
            addr: width
            for addr, width in candidates.items()
            if isinstance(addr, _Leaf) and addr.object in aggregates
        }
    usable = _available(body, candidates, dgroup, bounds)
    used = {_key(ref, blocked) for block in body.blocks for op in block.ops if id(op) in usable for ref in op.loads}
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
    asked = consts.memory_queries(body, {}, dgroup)
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
            after = consts._kills(memory.get((block.at, index), {}), op, {}, dgroup, calls, queries=asked)
            initialized[id(op)] = {
                addr: fact
                for addr, width in cells.items()
                if not isinstance(addr, (mir.MemRef, _Leaf))
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
    aggregate_only: bool = False,
) -> MirBody:
    """Reuse eligible stored values without removing observable writes."""
    original = body
    aggregate_objects = None
    if aggregate_only:
        refs = [ref for block in body.blocks for op in block.ops for ref in (*op.loads, *op.stores)]
        blocked = _blocked(refs)
        aggregate_objects = _aggregate_objects(
            key for ref in refs if (key := _key(ref, blocked)) is not None
        )
        if not aggregate_objects:
            return original
    body = _separated(body, aggregate_objects) if split_updates else body
    found = promotable(body, dgroup, bounds, aggregate_only=aggregate_only)
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
        return promoted(
            original,
            dgroup,
            bounds,
            loop_only=loop_only,
            split_updates=False,
            aggregate_only=aggregate_only,
        )

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
                    ops.append(
                        replace(
                            exact,
                            source_backed=False,
                            id=None,
                            absorbed=(),
                            symbol=False,
                        )
                    )
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
                            source_backed=False,
                            raised=None,
                            id=None,
                            absorbed=(),
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
                made = replace(
                    made,
                    source_backed=False,
                    id=None,
                    absorbed=(),
                    symbol=False,
                )
            ops.append(made)
        blocks.append(replace(block, ops=tuple(ops)))
    if not changed:
        return body
    return ssa.constructed(replace(body, blocks=tuple(blocks)), frozenset(holds.values()))


def _separated(body: MirBody, objects: frozenset[memory.Object] | None = None) -> MirBody:
    """Expose a memory update as a value computation and an observable store."""
    fresh = _next(body)
    variable = max((one.variable for one in ssa.values(body)), default=0) + 1
    blocks = []
    for block in body.blocks:
        ops = []
        for op in block.ops:
            leaf = _leaf(op.loads[0]) if len(op.loads) == 1 else None
            if (
                op.kind not in READS - {mir.Kind.LOAD}
                or len(op.loads) != 1
                or op.loads != op.stores
                or op.results != (mir.Cell(op.loads[0]),)
                or any(not value.flags for value in op.defines)
                or (
                    objects is None
                    and (op.loads[0].base is not None or op.loads[0].segment is not None)
                )
                or (objects is not None and (leaf is None or leaf.object not in objects))
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
                    uses=tuple(
                        value
                        for value in (result, op.loads[0].base, op.loads[0].segment)
                        if value is not None
                    ),
                    source_backed=False,
                    raised=None,
                    absorbed=(),
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
