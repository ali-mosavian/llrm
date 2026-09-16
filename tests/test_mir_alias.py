from dataclasses import replace

from iced_x86 import Register

from qbopt.model import ir
from qbopt.model import mir
from qbopt.model import memory
from qbopt.analysis import alias
from qbopt.objectfile.module import Addr
from qbopt.objectfile.module import Space


def test_equal_offsets_do_not_prove_far_segments_disjoint() -> None:
    """1000:0020 and 1001:0010 name the same byte despite different offsets."""
    base = mir.Value(1, 0)
    one = mir.MemRef(Addr(Space.FAR, 0x20, base=Register.BX), 2, base, mir.Value(2, 0))
    other = mir.MemRef(Addr(Space.FAR, 0x10, base=Register.BX), 2, base, mir.Value(3, 0))
    assert mir.overlapping(one, other, frozenset())
    assert mir.overlapping(other, one, frozenset())
    assert not mir.overlapping(one, replace(other, segment=one.segment), frozenset())


def test_unknown_far_segments_cannot_use_offset_disjointness() -> None:
    base = mir.Value(1, 0)
    one = mir.MemRef(Addr(Space.FAR, 0x20, base=Register.BX), 2, base)
    other = replace(one, addr=one.addr.plus(16))
    assert mir.overlapping(one, other, frozenset())


def test_an_index_stays_inside_its_own_segment() -> None:
    """Axiom 4 in regions: two segments' indexed cells never meet."""
    base = mir.Value(1, 0)
    one = mir.MemRef(Addr(Space.SEGMENT, 0x20, 1, Register.SI), 2, base)
    other = mir.MemRef(Addr(Space.SEGMENT, 0x10, 2, Register.SI), 2, base)
    assert not mir.overlapping(one, other, frozenset({1, 2}))


def test_canonical_subobjects_use_object_identity_and_byte_ranges() -> None:
    """Two fields of one object are disjoint; equal offsets in two objects are too."""
    first = memory.Object(memory.Kind.FRAME, (0, -8), extent=8)
    second = memory.Object(memory.Kind.FRAME, (0, -16), extent=8)
    a = mir.MemRef(None, 4, provenance=memory.Provenance.one(first, 0, 4))
    b = mir.MemRef(None, 4, provenance=memory.Provenance.one(first, 4, 8))
    c = mir.MemRef(None, 4, provenance=memory.Provenance.one(second, 0, 4))

    assert not mir.overlapping(a, b, frozenset())
    assert not mir.overlapping(a, c, frozenset())
    assert mir.overlapping(a, replace(a, width=2), frozenset())


def test_points_to_flows_through_memory_and_a_phi() -> None:
    """A pointer spilled on one arm and joined with a copy retains its object set."""
    root, loaded, joined = (mir.Value(n, n, variable=n, version=1) for n in range(1, 4))
    slot = mir.MemRef(Addr(Space.FRAME, -2), 2, space=Space.FRAME)
    address = mir.Op(
        1,
        kind=mir.Kind.ADDRESS,
        op=ir.Operation.ADDRESS,
        name="",
        defines=(root,),
        uses=(),
        args=(mir.FrameAddress(-8, 2, (-8, -4)),),
        results=(mir.Held(root, 2),),
    )
    store = mir.Op(
        2,
        kind=mir.Kind.STORE,
        op=ir.Operation.MOVE,
        name="",
        defines=(),
        uses=(root,),
        args=(mir.Held(root, 2),),
        results=(mir.Cell(slot),),
        stores=(slot,),
    )
    load = mir.Op(
        3,
        kind=mir.Kind.LOAD,
        op=ir.Operation.MOVE,
        name="",
        defines=(loaded,),
        uses=(),
        args=(mir.Cell(slot),),
        results=(mir.Held(loaded, 2),),
        loads=(slot,),
    )
    body = mir.MirBody(
        0,
        (
            mir.MirBlock(0, (), (address, store), (10, 20)),
            mir.MirBlock(10, (), (load,), (30,)),
            mir.MirBlock(20, (), (), (30,)),
            mir.MirBlock(30, (mir.Phi(joined, {10: loaded, 20: root}),), (), ()),
        ),
        pointer_values=frozenset({root, loaded, joined}),
    )

    facts = alias.points_to(body)
    assert facts.values[loaded] == facts.values[root]
    assert facts.values[joined] == facts.values[root]


def test_pointer_phi_with_an_unknown_arm_is_unknown() -> None:
    """A known arm must not erase the other arm: that falsely made the joined pointer disjoint from real objects."""
    known, unknown, joined = (mir.Value(n, n, variable=n, version=1) for n in range(1, 4))
    object_ = memory.Object(memory.Kind.FRAME, ("f", -4), extent=4)
    body = mir.MirBody(
        0,
        (
            mir.MirBlock(0, (), (), (10, 20)),
            mir.MirBlock(10, (), (), (30,)),
            mir.MirBlock(20, (), (), (30,)),
            mir.MirBlock(30, (mir.Phi(joined, {10: known, 20: unknown}),), (), ()),
        ),
        pointer_values=frozenset({known, unknown, joined}),
        pointer_seeds={known: memory.Provenance.one(object_, 0, 1)},
    )

    assert alias.points_to(body).values[joined] == alias.UNKNOWN


def test_parameter_modref_is_instantiated_at_a_call_site() -> None:
    """A callee writing parameter zero clobbers its actual object and no neighbour."""
    param = memory.Object(memory.Kind.PARAMETER, 0)
    summary = alias.Summary(writes=frozenset({memory.Slice(param, 2, 4)}))
    actual = memory.Object(memory.Kind.GLOBAL, (Space.SEGMENT, 7), extent=16)
    effect = summary.instantiated((memory.Provenance.one(actual, 4, 5),)).writes

    assert effect == frozenset({memory.Slice(actual, 6, 8)})


def test_interprocedural_modref_reaches_the_call_operation() -> None:
    """A known callee replaces CALL's catch-all effect with its actual object."""
    parameter = memory.Object(memory.Kind.PARAMETER, 0)
    actual = memory.Object(memory.Kind.GLOBAL, (Space.SEGMENT, 9), extent=16)
    write = mir.MemRef(None, 2, provenance=memory.Provenance.one(parameter, 2, 4))
    stored = mir.Op(1, ir.Operation.MOVE, "", (), (), kind=mir.Kind.STORE, stores=(write,))
    callee = alias.Procedure(mir.MirBody(0, (mir.MirBlock(0, (), (stored,), ()),)), {}, {})

    called = mir.Op(
        2,
        ir.Operation.CALL,
        "",
        (),
        (),
        kind=mir.Kind.CALL,
        loads=(mir.MemRef(None, 4),),
        stores=(mir.MemRef(None, 4),),
    )
    caller = alias.Procedure(
        mir.MirBody(0, (mir.MirBlock(0, (), (called,), ()),)),
        {2: "callee"},
        {2: (memory.Provenance.one(actual, 4, 5),)},
    )
    known = alias.summaries({"caller": caller, "callee": callee})
    body = alias.calls_annotated(caller, known)
    effect = body.blocks[0].ops[0]

    assert effect.memory_complete and effect.loads == ()
    assert effect.stores[0].provenance == memory.Provenance.one(actual, 6, 8)


def test_recursive_pointer_offset_summary_widens_and_terminates() -> None:
    """A recursive f(p + 1) grew one byte per summary round; the SCC effect is the whole formal object."""
    pointer = mir.Value(1, 1, variable=1, version=1)
    parameter = memory.Object(memory.Kind.PARAMETER, 0)
    write = mir.MemRef(None, 1, provenance=memory.Provenance.one(parameter, 0, 1))
    store = mir.Op(1, ir.Operation.MOVE, "", (), (), kind=mir.Kind.STORE, stores=(write,))
    call = mir.Op(2, ir.Operation.CALL, "", (), (), kind=mir.Kind.CALL)
    body = mir.MirBody(
        0,
        (mir.MirBlock(0, (), (store, call), ()),),
        pointer_values=frozenset({pointer}),
        pointer_seeds={pointer: memory.Provenance.one(parameter, 0, 1)},
    )
    procedure = alias.Procedure(body, {2: "recursive"}, {2: ((pointer, 1),)})

    summary = alias.summaries({"recursive": procedure})["recursive"]

    assert summary.writes == frozenset({memory.Slice(parameter)})


def test_strided_ranges_prove_interleaved_arrays_disjoint() -> None:
    """Even and odd byte lanes have overlapping hulls but no common byte."""
    obj = memory.Object(memory.Kind.GLOBAL, (Space.SEGMENT, 4), extent=64)
    even = mir.MemRef(None, 1, provenance=memory.Provenance.one(obj, 0, 64, stride=2))
    odd = mir.MemRef(None, 1, provenance=memory.Provenance.one(obj, 1, 64, stride=2))

    assert not mir.overlapping(even, odd, frozenset())


def test_restrict_roots_and_tbaa_share_the_alias_query() -> None:
    """Distinct restrict roots are disjoint; character accesses still defeat TBAA."""
    unknown = memory.Object(memory.Kind.UNKNOWN)
    left = mir.MemRef(None, 4, typed=("int4", False), provenance=memory.Provenance.one(unknown, restrict=1))
    right = mir.MemRef(None, 4, typed=("float4", False), provenance=memory.Provenance.one(unknown, restrict=2))
    chars = replace(right, typed=None, provenance=memory.Provenance.one(unknown))
    typed_only = replace(right, provenance=memory.Provenance.one(unknown))

    assert not mir.overlapping(left, right, frozenset())
    assert not mir.overlapping(replace(left, provenance=memory.Provenance.one(unknown)), typed_only, frozenset())
    assert mir.overlapping(left, chars, frozenset())


def test_unknown_call_reaches_nonlocals_and_only_its_pointer_actual() -> None:
    """An unknown C call does not clobber every local, but may use a local whose address it receives."""
    passed = memory.Object(memory.Kind.FRAME, ("caller", -8), extent=4)
    private = memory.Object(memory.Kind.FRAME, ("caller", -12), extent=4)
    call = mir.Op(
        4,
        ir.Operation.CALL,
        "",
        (),
        (),
        kind=mir.Kind.CALL,
        loads=(mir.MemRef(None, 4),),
        stores=(mir.MemRef(None, 4),),
    )
    procedure = alias.Procedure(
        mir.MirBody(0, (mir.MirBlock(0, (), (call,), ()),)),
        {4: "external"},
        {4: (memory.Provenance.one(passed, 0, 1), None)},
    )

    annotated = alias.calls_annotated(procedure, {})
    effect = annotated.blocks[0].ops[0]
    passed_ref = mir.MemRef(None, 4, provenance=memory.Provenance.one(passed, 0, 4))
    passed_tail = mir.MemRef(None, 1, provenance=memory.Provenance.one(passed, 3, 4))
    private_ref = mir.MemRef(None, 4, provenance=memory.Provenance.one(private, 0, 4))

    assert effect.memory_complete
    assert any(mir.overlapping(passed_ref, one, frozenset()) for one in effect.stores)
    assert any(mir.overlapping(passed_tail, one, frozenset()) for one in effect.stores)
    assert not any(mir.overlapping(private_ref, one, frozenset()) for one in effect.stores)


def test_unknown_call_reaches_a_frame_pointer_escaped_before_the_call() -> None:
    """Storing &local outside the frame exposes that object to a later unknown call, not its neighbours."""
    pointer = mir.Value(1, 1, variable=1, version=1)
    escaped = memory.Object(memory.Kind.FRAME, ("caller", -8), extent=4)
    private = memory.Object(memory.Kind.FRAME, ("caller", -12), extent=4)
    global_ = memory.Object(memory.Kind.GLOBAL, (Space.SEGMENT, 9))
    destination = mir.MemRef(None, 2, provenance=memory.Provenance.one(global_, 0, 2))
    publish = mir.Op(
        2,
        ir.Operation.MOVE,
        "",
        (),
        (pointer,),
        kind=mir.Kind.STORE,
        args=(mir.Held(pointer, 2),),
        stores=(destination,),
    )
    call = mir.Op(4, ir.Operation.CALL, "", (), (), kind=mir.Kind.CALL)
    body = mir.MirBody(
        0,
        (mir.MirBlock(0, (), (publish, call), ()),),
        pointer_values=frozenset({pointer}),
        pointer_seeds={pointer: memory.Provenance.one(escaped, 0, 1)},
    )
    procedure = alias.Procedure(body, {4: "external"}, {4: ()})

    effect = alias.calls_annotated(procedure, {}).blocks[0].ops[-1]
    escaped_ref = mir.MemRef(None, 4, provenance=memory.Provenance.one(escaped, 0, 4))
    private_ref = mir.MemRef(None, 4, provenance=memory.Provenance.one(private, 0, 4))

    assert any(mir.overlapping(escaped_ref, one, frozenset()) for one in effect.stores)
    assert not any(mir.overlapping(private_ref, one, frozenset()) for one in effect.stores)


def test_known_capture_summary_controls_later_external_reach() -> None:
    """A borrowed pointer remains private; a retained pointer exposes its object to subsequent unknown calls."""
    pointer = mir.Value(1, 1, variable=1, version=1)
    local = memory.Object(memory.Kind.FRAME, ("caller", -8), extent=4)
    first = mir.Op(2, ir.Operation.CALL, "", (), (), kind=mir.Kind.CALL)
    second = mir.Op(3, ir.Operation.CALL, "", (), (), kind=mir.Kind.CALL)
    body = mir.MirBody(
        0,
        (mir.MirBlock(0, (), (first, second), ()),),
        pointer_values=frozenset({pointer}),
        pointer_seeds={pointer: memory.Provenance.one(local, 0, 1)},
    )
    procedure = alias.Procedure(body, {2: "known", 3: "external"}, {2: ((pointer, 0),), 3: ()})
    reference = mir.MemRef(None, 4, provenance=memory.Provenance.one(local, 0, 4))

    borrowed = alias.calls_annotated(procedure, {"known": alias.Summary()}).blocks[0].ops[-1]
    captured = alias.calls_annotated(procedure, {"known": alias.Summary(captures=frozenset({0}))}).blocks[0].ops[-1]

    assert not any(mir.overlapping(reference, one, frozenset()) for one in borrowed.stores)
    assert any(mir.overlapping(reference, one, frozenset()) for one in captured.stores)


def test_promotion_keeps_canonical_identity_across_a_nonlocal_call() -> None:
    """calls.c reloaded k: promotion discarded its object identity before asking whether the call aliased it."""
    from qbopt.optimize import promote

    local = memory.Object(memory.Kind.FRAME, ("caller", -4), extent=2)
    cell = mir.MemRef(
        Addr(Space.FRAME, -4),
        2,
        space=Space.FRAME,
        provenance=memory.Provenance.one(local, 0, 2),
    )
    nonlocal_ = mir.MemRef(None, 1, provenance=alias.NONLOCAL)
    value = mir.Value(1, 1, variable=1, version=1)
    store = mir.Op(
        1,
        ir.Operation.MOVE,
        "",
        (),
        (),
        kind=mir.Kind.STORE,
        args=(mir.Const(0, 2),),
        results=(mir.Cell(cell),),
        stores=(cell,),
    )
    call = mir.Op(
        2,
        ir.Operation.CALL,
        "",
        (),
        (),
        kind=mir.Kind.CALL,
        loads=(nonlocal_,),
        stores=(nonlocal_,),
        memory_complete=True,
    )
    load = mir.Op(
        3,
        ir.Operation.MOVE,
        "",
        (value,),
        (),
        kind=mir.Kind.LOAD,
        args=(mir.Cell(cell),),
        results=(mir.Held(value, 2),),
        loads=(cell,),
    )
    body = mir.MirBody(0, (mir.MirBlock(0, (), (store, call, load), ()),))

    assert list(promote.promotable(body).values()) == [2]


def test_index_interval_counts_an_access_width_once() -> None:
    """A one-element dword range ended at byte 4, but widening its start range again made it overlap field 4."""
    from qbopt.analysis.ranges import Interval

    index = mir.Value(1, 1, variable=1, version=1)
    object_ = memory.Object(memory.Kind.GLOBAL, (Space.SEGMENT, 3), extent=16)
    indexed = mir.MemRef(
        Addr(Space.SEGMENT, 0, 3),
        4,
        base=index,
        base_width=2,
        provenance=memory.Provenance.one(object_),
    )
    next_field = mir.MemRef(None, 4, provenance=memory.Provenance.one(object_, 4, 8))

    assert not mir.overlapping(indexed, next_field, frozenset(), known={index: Interval(0, 0, 2)})


def test_strided_slice_intersection_matches_the_bytes_it_describes() -> None:
    """Dependence proofs use modular lanes; exhaust the small cases against their literal byte sets."""
    object_ = memory.Object(memory.Kind.ALLOCATION, 1, extent=12)

    def bytes_of(one):
        return {start + lane for start in range(one.low, one.high, one.stride) for lane in range(one.width)}

    slices = [
        memory.Slice(object_, low, high, stride, width)
        for low in range(4)
        for high in range(low + 1, 7)
        for stride in range(1, 5)
        for width in range(1, 4)
    ]
    for one in slices:
        for other in slices:
            assert one.intersects(other) == bool(bytes_of(one) & bytes_of(other)), (one, other)
