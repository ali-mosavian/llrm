"""Strong, flow-sensitive alias analysis over MIR.

The analysis has one vocabulary for every frontend: canonical objects,
subobject byte slices, pointer provenance and C restrict roots.  Pointer facts
flow through SSA phis and through exact pointer spill slots.  Unknown stores
kill spill facts; they never manufacture a disjointness proof.
"""

from math import gcd
from dataclasses import field
from dataclasses import replace
from dataclasses import dataclass

from qbopt.model import mir
from qbopt.model import memory
from qbopt.objectfile.module import Space

UNKNOWN = memory.Provenance.one(memory.Object(memory.Kind.UNKNOWN))
NONLOCAL = memory.Provenance.one(memory.Object(memory.Kind.NONLOCAL))
EMPTY = memory.Provenance(frozenset())


def _whole(provenances, escaped=()) -> set[memory.Slice]:
    """Whole objects an unknown callee can reach through pointers it owns."""
    objects = {one.object for provenance in provenances for one in provenance.slices} | set(escaped)
    return {
        memory.Slice(object_, 0, object_.extent) if object_.extent is not None else memory.Slice(object_)
        for object_ in objects
    }


@dataclass(frozen=True, slots=True)
class PointsTo:
    values: dict[mir.Value, memory.Provenance]
    escaped: frozenset[memory.Object] = frozenset()
    # Objects visible immediately before each source operation address.
    escaped_before: dict[int, frozenset[memory.Object]] = field(default_factory=dict)

    def reference(self, ref: mir.MemRef) -> memory.Provenance | None:
        """Canonical bytes reached by a reference through an analysed value."""
        return _resolved_reference(ref, self.values)


def _resolved_reference(
    ref: mir.MemRef,
    values: dict[mir.Value, memory.Provenance],
) -> memory.Provenance | None:
    """Resolve a memory operand through the current pointer-value facts."""
    if ref.provenance is not None:
        return ref.provenance
    if ref.base is None or ref.base not in values:
        return None
    source = values[ref.base]
    displacement = ref.addr.disp if ref.addr is not None else 0
    slices = set()
    for one in source.slices:
        low, high = one.low + displacement, one.high + displacement
        # A singleton address names `width` consecutive bytes. A set of
        # indexed addresses retains its stride and widens its final lane.
        slices.add(memory.Slice(one.object, low, high, one.stride, max(ref.width, 1)))
    return memory.Provenance(frozenset(slices), source.restrict)


@dataclass(frozen=True, slots=True)
class Summary:
    """One procedure's transitive memory effects in its own object space."""

    reads: frozenset[memory.Slice] = frozenset()
    writes: frozenset[memory.Slice] = frozenset()
    captures: frozenset[int] = frozenset()
    unknown_read: bool = False
    unknown_write: bool = False

    def instantiated(self, arguments: tuple[memory.Provenance, ...]) -> "Summary":
        def expand(items: frozenset[memory.Slice]) -> frozenset[memory.Slice]:
            out = set()
            for item in items:
                if item.object.kind is not memory.Kind.PARAMETER:
                    out.add(item)
                    continue
                index = item.object.identity
                if not isinstance(index, int) or not 0 <= index < len(arguments):
                    continue
                for actual in arguments[index].slices:
                    out.add(
                        memory.Slice(
                            actual.object,
                            actual.low + item.low,
                            actual.high + item.high - 1,
                            gcd(actual.stride, item.stride),
                            item.width,
                        )
                    )
            return frozenset(out)

        return Summary(
            expand(self.reads),
            expand(self.writes),
            self.captures,
            self.unknown_read,
            self.unknown_write,
        )


@dataclass(frozen=True, slots=True)
class Procedure:
    body: mir.MirBody
    calls: dict[int, str]
    arguments: dict[int, tuple[object, ...]]


def _actuals(procedure: Procedure, facts: PointsTo, at: int) -> tuple[memory.Provenance, ...]:
    out = []
    for actual in procedure.arguments.get(at, ()):
        if isinstance(actual, memory.Provenance):
            out.append(actual)
        elif (
            isinstance(actual, tuple)
            and len(actual) == 2
            and isinstance(actual[0], mir.Value)
            and actual[0] in facts.values
        ):
            out.append(facts.values[actual[0]].shifted(actual[1]))
        elif actual is None:
            out.append(EMPTY)
        else:
            out.append(UNKNOWN)
    return tuple(out)


def _direct_summary(body: mir.MirBody) -> Summary:
    reads, writes = set(), set()
    unknown_read = unknown_write = False
    facts = points_to(body)
    for block in body.blocks:
        for op in block.ops:
            if op.kind is mir.Kind.CALL:
                continue
            for ref, destination in (*((ref, reads) for ref in op.loads), *((ref, writes) for ref in op.stores)):
                provenance = facts.reference(ref)
                if provenance is None:
                    if destination is reads:
                        unknown_read = True
                    else:
                        unknown_write = True
                    continue
                for one in provenance.slices:
                    if one.object.kind not in (memory.Kind.FRAME, memory.Kind.STACK):
                        destination.add(one)
                    if one.object.kind is memory.Kind.UNKNOWN:
                        if destination is reads:
                            unknown_read = True
                        else:
                            unknown_write = True
    captures = frozenset(
        one.identity for one in facts.escaped if one.kind is memory.Kind.PARAMETER and isinstance(one.identity, int)
    )
    return Summary(frozenset(reads), frozenset(writes), captures, unknown_read, unknown_write)


def _recursive_edges(procedures: dict[str, Procedure]) -> frozenset[tuple[str, str]]:
    graph = {
        name: {target for target in procedure.calls.values() if target in procedures}
        for name, procedure in procedures.items()
    }

    def reaches(start: str, wanted: str) -> bool:
        pending, seen = [start], set()
        while pending:
            at = pending.pop()
            if at == wanted:
                return True
            if at in seen:
                continue
            seen.add(at)
            pending.extend(graph.get(at, ()))
        return False

    return frozenset(
        (caller, callee) for caller, targets in graph.items() for callee in targets if reaches(callee, caller)
    )


def _widen_parameters(summary: Summary) -> Summary:
    def widened(items: frozenset[memory.Slice]) -> frozenset[memory.Slice]:
        return frozenset(memory.Slice(one.object) if one.object.kind is memory.Kind.PARAMETER else one for one in items)

    return replace(summary, reads=widened(summary.reads), writes=widened(summary.writes))


def _coalesced(items) -> frozenset[memory.Slice]:
    """Drop subranges once the same object already has a whole-object effect."""
    whole = {
        one.object for one in items if one.low == -(1 << 31) and one.high == 1 << 31 and one.stride == one.width == 1
    }
    return frozenset(
        one
        for one in items
        if one.object not in whole or one.low == -(1 << 31) and one.high == 1 << 31 and one.stride == one.width == 1
    )


def summaries(
    procedures: dict[str, Procedure],
    known: dict[str, Summary] | None = None,
) -> dict[str, Summary]:
    """Transitive per-procedure mod/ref and capture summaries to a fixed point.

    ``known`` supplies established external semantics, such as C library
    functions. A body in this compilation unit always takes precedence.
    """
    result = {**(known or {}), **{name: _direct_summary(one.body) for name, one in procedures.items()}}
    recursive = _recursive_edges(procedures)
    while True:
        changed = False
        for name, procedure in procedures.items():
            captured_at = {
                at: result[target].captures if target in result else None for at, target in procedure.calls.items()
            }
            facts = points_to(procedure.body, procedure.arguments, captured_at)
            direct = _direct_summary(procedure.body)
            reads, writes, captures = set(direct.reads), set(direct.writes), set(direct.captures)
            unknown_read, unknown_write = direct.unknown_read, direct.unknown_write
            for block in procedure.body.blocks:
                for op in block.ops:
                    if op.kind is not mir.Kind.CALL:
                        continue
                    target = procedure.calls.get(op.at)
                    callee = result.get(target)
                    actual = _actuals(procedure, facts, op.at)
                    if callee is None:
                        visible = set(NONLOCAL.slices)
                        visible.update(_whole(actual, facts.escaped_before.get(op.at, ())))
                        reads.update(visible)
                        writes.update(visible)
                        captures.update(
                            slice_.object.identity
                            for provenance in actual
                            for slice_ in provenance.slices
                            if slice_.object.kind is memory.Kind.PARAMETER
                        )
                        continue
                    effect = callee.instantiated(actual)
                    if (name, target) in recursive:
                        effect = _widen_parameters(effect)
                    reads.update(effect.reads)
                    writes.update(effect.writes)
                    unknown_read |= effect.unknown_read
                    unknown_write |= effect.unknown_write
                    for index in effect.captures:
                        if 0 <= index < len(actual):
                            captures.update(
                                one.object.identity
                                for one in actual[index].slices
                                if one.object.kind is memory.Kind.PARAMETER
                            )
            made = Summary(_coalesced(reads), _coalesced(writes), frozenset(captures), unknown_read, unknown_write)
            if made != result[name]:
                result[name] = made
                changed = True
        if not changed:
            return result


def calls_annotated(
    procedure: Procedure,
    known: dict[str, Summary],
) -> mir.MirBody:
    """Instantiate callee effects through actual pointer provenance."""
    # Capture is part of escape flow. Unknown callees may retain every
    # pointer actual; known callees retain only the parameters their fixed
    # point summary says they capture.
    captures = {at: known[target].captures if target in known else None for at, target in procedure.calls.items()}
    facts = points_to(procedure.body, procedure.arguments, captures)

    def reference(one: memory.Slice) -> mir.MemRef:
        provenance = memory.Provenance(frozenset({one}))
        return mir.MemRef(None, one.width, provenance=provenance)

    blocks = []
    for block in procedure.body.blocks:
        ops = []
        for op in block.ops:
            if op.kind is not mir.Kind.CALL:
                ops.append(op)
                continue
            actual = _actuals(procedure, facts, op.at)
            target = procedure.calls.get(op.at)
            if target in known:
                effect = known[target].instantiated(actual)
            else:
                visible = set(NONLOCAL.slices)
                visible.update(_whole(actual, facts.escaped_before.get(op.at, ())))
                effect = Summary(frozenset(visible), frozenset(visible))
            if effect.unknown_read:
                effect = replace(effect, reads=effect.reads | UNKNOWN.slices)
            if effect.unknown_write:
                effect = replace(effect, writes=effect.writes | UNKNOWN.slices)
            loads = tuple(reference(one) for one in sorted(effect.reads, key=repr))
            stores = tuple(reference(one) for one in sorted(effect.writes, key=repr))
            ops.append(
                replace(
                    op,
                    loads=loads,
                    stores=stores,
                    memory_complete=True,
                )
            )
        blocks.append(replace(block, ops=tuple(ops)))
    return replace(procedure.body, blocks=tuple(blocks))


def _union(parts) -> memory.Provenance | None:
    values = [one for one in parts if one is not None]
    if not values:
        return None
    result = values[0]
    for one in values[1:]:
        result = result.union(one)
    return result


def _cell_key(ref: mir.MemRef):
    if ref.provenance is not None and len(ref.provenance.slices) == 1:
        one = next(iter(ref.provenance.slices))
        if one.stride == 1:
            return (one.object, one.low, one.high)
    if ref.addr is not None and ref.base is None and ref.segment is None:
        return (ref.addr.space, ref.addr.index, ref.addr.disp, ref.width)
    return None


def _direct(op: mir.Op, values: dict[mir.Value, memory.Provenance]) -> memory.Provenance | None:
    if len(op.results) != 1 or not isinstance(op.results[0], mir.Held):
        return None
    match op.kind, op.args:
        case mir.Kind.ADDRESS, (mir.FrameAddress(offset=offset, extent=extent),) if extent is not None:
            low, high = extent
            object_ = memory.Object(memory.Kind.FRAME, (low, high), extent=high - low)
            return memory.Provenance.one(object_, offset - low, offset - low + 1)
        case mir.Kind.COPY, (mir.Symbol(space=space, index=index, offset=offset, addend=addend),):
            kind = memory.Kind.EXTERNAL if space is Space.EXTERNAL else memory.Kind.GLOBAL
            return memory.Provenance.one(memory.Object(kind, (space, index)), offset + addend, offset + addend + 1)
        case mir.Kind.COPY, (mir.Held(value=source),):
            return values.get(source)
        case mir.Kind.ADD | mir.Kind.PTR_OFFSET, (left, right):
            candidates = ((left, right), (right, left))
        case mir.Kind.SUB, (left, right):
            candidates = ((left, right),)
        case _:
            return None
    for pointer, amount in candidates:
        if not isinstance(pointer, mir.Held) or pointer.value not in values:
            continue
        fact = values[pointer.value]
        if isinstance(amount, mir.Const):
            delta = amount.n if op.kind is not mir.Kind.SUB else -amount.n
            return fact.shifted(delta)
        # Arithmetic by an unknown integer remains within each known object,
        # but no longer has a byte offset precise enough to compare.
        slices = set()
        for one in fact.slices:
            high = one.object.extent
            slices.add(memory.Slice(one.object, 0, high) if high is not None else memory.Slice(one.object))
        return memory.Provenance(frozenset(slices), fact.restrict)
    return None


def points_to(
    body: mir.MirBody,
    arguments: dict[int, tuple[object, ...]] | None = None,
    captures: dict[int, frozenset[int] | None] | None = None,
) -> PointsTo:
    """Flow pointer objects through values, exact spill slots and CFG joins."""
    values = dict(body.pointer_seeds)
    pointer_values = set(body.pointer_values) | set(values)
    predecessors = {block.at: set() for block in body.blocks}
    for block in body.blocks:
        for successor in block.succ:
            predecessors.setdefault(successor, set()).add(block.at)
    incoming: dict[int, dict] = {block.at: {} for block in body.blocks}
    outgoing: dict[int, dict] = {block.at: {} for block in body.blocks}

    while True:
        before_values = dict(values)
        before_outgoing = {at: dict(state) for at, state in outgoing.items()}
        for block in body.blocks:
            parents = [outgoing[one] for one in predecessors.get(block.at, ())]
            state = {}
            if parents:
                keys = set().union(*(one.keys() for one in parents))
                for key in keys:
                    # A missing fact on one incoming edge is unknown, not an
                    # invitation to retain the other edge's pointer.
                    if all(key in one for one in parents):
                        state[key] = _union(one[key] for one in parents)
            incoming[block.at] = dict(state)
            for phi in block.phis:
                if phi.result in pointer_values:
                    parts = [values.get(one) for one in phi.incoming.values()]
                    fact = UNKNOWN if any(one is None for one in parts) else _union(parts)
                    if fact is not None:
                        values[phi.result] = fact
            for op in block.ops:
                direct = _direct(op, values)
                for result in op.defines:
                    if result in pointer_values and result not in body.pointer_seeds and direct is not None:
                        values[result] = direct
                if op.loads and len(op.defines) == 1 and op.defines[0] in pointer_values:
                    facts = [state.get(_cell_key(ref)) for ref in op.loads]
                    loaded = _union(facts)
                    if loaded is not None:
                        values[op.defines[0]] = loaded
                if op.stores:
                    source = _union(values.get(arg.value) for arg in op.args if isinstance(arg, mir.Held))
                    for ref in op.stores:
                        provenance = _resolved_reference(ref, values)
                        key = _cell_key(replace(ref, provenance=provenance))
                        # Any possibly overlapping write invalidates prior cell
                        # contents; an exact pointer store then defines it.
                        state = {old: fact for old, fact in state.items() if old == key or not _keys_overlap(old, key)}
                        if key is not None and source is not None:
                            state[key] = source
            outgoing[block.at] = state
        if values == before_values and outgoing == before_outgoing:
            break

    # Escape is flow-sensitive separately from pointer contents. A pointer
    # published after a call must not make the earlier call reach its frame.
    escape_in: dict[int, set[memory.Object]] = {block.at: set() for block in body.blocks}
    escape_out: dict[int, set[memory.Object]] = {block.at: set() for block in body.blocks}
    escaped_before: dict[int, frozenset[memory.Object]] = {}
    while True:
        before = {at: set(one) for at, one in escape_out.items()}
        for block in body.blocks:
            state = set().union(*(escape_out[one] for one in predecessors.get(block.at, ())))
            escape_in[block.at] = set(state)
            for op in block.ops:
                escaped_before[op.at] = frozenset(set(escaped_before.get(op.at, ())) | state)
                newly = set()
                if op.kind is mir.Kind.CALL and arguments is not None:
                    actual = _resolved_actuals(arguments.get(op.at, ()), values)
                    selected = (captures or {}).get(op.at)
                    selected = range(len(actual)) if selected is None else selected
                    newly.update(
                        one.object for index in selected if 0 <= index < len(actual) for one in actual[index].slices
                    )
                if op.kind in (mir.Kind.RETURN, mir.Kind.ESCAPE):
                    for value in op.uses:
                        if value in values:
                            newly.update(one.object for one in values[value].slices)
                if op.stores:
                    destinations = [
                        provenance for ref in op.stores if (provenance := _resolved_reference(ref, values)) is not None
                    ]
                    outside = not destinations or any(
                        one.object.kind is not memory.Kind.FRAME
                        for provenance in destinations
                        for one in provenance.slices
                    )
                    if outside:
                        for arg in op.args:
                            if isinstance(arg, mir.Held) and arg.value in values:
                                newly.update(one.object for one in values[arg.value].slices)
                state.update(newly)
            escape_out[block.at] = state
        if escape_out == before:
            break
    escaped = frozenset().union(*(frozenset(one) for one in escape_out.values()))
    return PointsTo(values, escaped, escaped_before)


def _resolved_actuals(actuals: tuple[object, ...], values: dict[mir.Value, memory.Provenance]):
    out = []
    for actual in actuals:
        if isinstance(actual, memory.Provenance):
            out.append(actual)
        elif isinstance(actual, tuple) and len(actual) == 2 and isinstance(actual[0], mir.Value):
            out.append(values.get(actual[0], UNKNOWN).shifted(actual[1]))
        elif actual is None:
            out.append(EMPTY)
        else:
            out.append(UNKNOWN)
    return tuple(out)


def _keys_overlap(one, other) -> bool:
    if one is None or other is None:
        return True
    if len(one) == len(other) == 4 and one[:2] == other[:2]:
        return one[2] < other[2] + other[3] and other[2] < one[2] + one[3]
    if len(one) == len(other) == 3 and one[0] == other[0]:
        return one[1] < other[2] and other[1] < one[2]
    return one == other


def congruences(body: mir.MirBody) -> dict[mir.Value, tuple[int, int]]:
    """Known ``value == residue (mod modulus)`` facts; modulus zero is exact."""
    from qbopt.analysis import loops
    from qbopt.analysis import consts
    from qbopt.analysis import induction

    constants = consts.known(body)
    values = {value.id: value for value in body.values}
    result: dict[mir.Value, tuple[int, int]] = {}

    def number(arg):
        if isinstance(arg, mir.Const):
            return arg.n
        if isinstance(arg, mir.Held) and arg.value in constants:
            return constants[arg.value].n
        return None

    for loop in loops.loops(body.blocks, body.entry):
        for affine in induction.basics(body, loop).values():
            start, step = number(affine.start), number(affine.step)
            value = values.get(affine.value)
            if value is not None and start is not None and step not in (None, 0):
                modulus = abs(step)
                result[value] = (modulus, start % modulus)

    def computed(op: mir.Op):
        if len(op.results) != 1 or not isinstance(op.results[0], mir.Held) or op.loads or op.stores or op.barrier:
            return None
        args = op.args
        if op.kind is mir.Kind.COPY and len(args) == 1:
            n = number(args[0])
            if n is not None:
                return 0, n
            return result.get(args[0].value) if isinstance(args[0], mir.Held) else None
        if len(args) != 2:
            return None
        left, right = args
        a = (
            result.get(left.value)
            if isinstance(left, mir.Held)
            else (0, left.n)
            if isinstance(left, mir.Const)
            else None
        )
        b = (
            result.get(right.value)
            if isinstance(right, mir.Held)
            else (0, right.n)
            if isinstance(right, mir.Const)
            else None
        )
        if a is None or b is None:
            return None
        if op.kind in (mir.Kind.ADD, mir.Kind.SUB):
            modulus = gcd(a[0], b[0])
            residue = a[1] + (b[1] if op.kind is mir.Kind.ADD else -b[1])
            return modulus, residue if modulus == 0 else residue % modulus
        if op.kind is mir.Kind.MUL:
            if a[0] == 0:
                a, b = b, a
            if b[0] == 0:
                factor = b[1]
                modulus = abs(a[0] * factor)
                residue = a[1] * factor
                return modulus, residue if modulus == 0 else residue % modulus
        if op.kind is mir.Kind.SHL and b[0] == 0 and 0 <= b[1] < op.results[0].width * 8:
            factor = 1 << b[1]
            modulus = a[0] * factor
            return modulus, (a[1] * factor) % modulus if modulus else a[1] * factor
        return None

    while True:
        changed = False
        for block in body.blocks:
            for op in block.ops:
                fact = computed(op)
                if fact is None:
                    continue
                for value in op.defines:
                    if value not in result:
                        result[value] = fact
                        changed = True
        if not changed:
            return result


def annotated(body: mir.MirBody) -> mir.MirBody:
    """Attach solved provenance to every indirect reference in a body."""
    facts = points_to(body)
    from qbopt.analysis import ranges

    bounded = ranges.bounded(body)
    constants = ranges.constants(body)
    strides = congruences(body)

    def tag(ref: mir.MemRef, at: int) -> mir.MemRef:
        got = facts.reference(ref)
        interval = bounded.get(at, {}).get(ref.base) or constants.get(ref.base)
        if (
            got is not None
            and ref.provenance is not None
            and ref.base is not None
            and ref.addr is not None
            and interval is not None
            and interval.width == ref.base_width
            and len(got.slices) == 1
        ):
            source = next(iter(got.slices))
            modulus, residue = strides.get(ref.base, (1, 0))
            stride = modulus if modulus > 1 else 1
            first = interval.low + ((residue - interval.low) % stride)
            low, high = ref.addr.disp + first, ref.addr.disp + interval.high + 1
            end = high + max(ref.width, 1) - 1
            if low < high and (source.object.extent is None or 0 <= low < high and end <= source.object.extent):
                got = memory.Provenance(
                    frozenset({memory.Slice(source.object, low, high, stride, max(ref.width, 1))}),
                    got.restrict,
                )
        return replace(ref, provenance=got) if got is not None and got != ref.provenance else ref

    def operand(arg, at):
        return mir.Cell(tag(arg.ref, at)) if isinstance(arg, mir.Cell) else arg

    blocks = tuple(
        replace(
            block,
            ops=tuple(
                replace(
                    op,
                    loads=tuple(tag(ref, block.at) for ref in op.loads),
                    stores=tuple(tag(ref, block.at) for ref in op.stores),
                    args=tuple(operand(arg, block.at) for arg in op.args),
                    results=tuple(operand(arg, block.at) for arg in op.results),
                )
                for op in block.ops
            ),
        )
        for block in body.blocks
    )
    return replace(body, blocks=blocks)
