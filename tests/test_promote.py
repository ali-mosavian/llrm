"""Write-through promotion reuses proven values while preserving other memory accesses."""

from pathlib import Path
from dataclasses import replace

import pytest

from qbopt import wholeseg
from qbopt.model import mir
from qbopt.objectfile import omf
from qbopt.frontend import blocks
from qbopt.optimize import promote
from qbopt.objectfile import module
from qbopt.model.passes import Options


@pytest.mark.parametrize("tag", ["q-O", "p-g2", "v-g3"])
def test_guarded_indexed_accumulators_do_not_reload_in_loop(tag):
    """UDTRNG reloaded both LONG record fields on each of seven accumulator updates."""
    from qbopt.analysis import loops

    states = []

    def watch(stage, name, body):
        if isinstance(body, mir.MirBody):
            states.append(body)
    result = wholeseg.emitted(Path(f"fixtures/regressions/udtrng-{tag}.obj".lower()).read_bytes(), watch=watch)
    assert result.outcome is wholeseg.Emission.LIR, result.reason
    body = states[-1]
    hot = {at for loop in loops.loops(body.blocks, body.entry) for at in loop.body}
    assert not [
        (op.at, ref)
        for block in body.blocks
        if block.at in hot
        for op in block.ops
        for ref in op.loads
        if ref.base is not None and ref.width == 4
    ]


def test_procedure_frame_fields_reuse_stored_values():
    """LOCALP reread its frame accumulator on every addition despite known stores."""
    from qbopt.objectfile.module import Space

    path = Path("fixtures/regressions/localp-p-g2.obj")
    found = module.of(omf.parse(path.read_bytes()))
    body = next(
        body for name, body in mir.bodies(found, blocks.partition(found, blocks.code_map(found))) if body.entry != 0x30
    )
    before = next(
        op
        for block in body.blocks
        for op in block.ops
        if op.kind is mir.Kind.LOAD and op.loads and op.loads[0].width == 4 and op.loads[0].addr.space is Space.FRAME
    )
    result = promote.promoted(body, found.dgroup, module.landmarks(found), loop_only=True)
    after = next(op for block in result.blocks for op in block.ops if op.id == before.id)
    assert not after.loads
    assert all(not isinstance(arg, mir.Cell) for arg in after.args)


@pytest.mark.parametrize("clobber,reused", [(None, False), (-8, False), (-6, True)])
def test_frame_promotion_respects_unknown_and_overlapping_writes(clobber, reused):
    """LOCALP's held frame field must not survive an unknown call or a write to that field."""
    from qbopt.model import ir
    from qbopt.objectfile.module import Addr
    from qbopt.objectfile.module import Space

    cell = mir.MemRef(Addr(Space.FRAME, -8), 2)
    changed = mir.MemRef(None, 0) if clobber is None else mir.MemRef(Addr(Space.FRAME, clobber), 2)
    value = mir.Value(1, 4, variable=1, version=1)
    store = mir.Op(
        0,
        ir.Operation.MOVE,
        "",
        (),
        (),
        kind=mir.Kind.STORE,
        args=(mir.Const(7, 2),),
        results=(mir.Cell(cell),),
        stores=(cell,),
    )
    write = mir.Op(
        2,
        ir.Operation.CALL,
        "",
        (),
        (),
        kind=mir.Kind.CALL,
        stores=(changed,),
        memory_complete=clobber is not None,
    )
    load = mir.Op(
        4,
        ir.Operation.MOVE,
        "",
        (value,),
        (),
        kind=mir.Kind.LOAD,
        args=(mir.Cell(cell),),
        results=(mir.Held(value, 2),),
        loads=(cell,),
    )
    body = mir.MirBody(0, (mir.MirBlock(0, (), (store, write, load), ()),))
    result = promote.promoted(body)
    after = next(op for block in result.blocks for op in block.ops if op.at == 4)
    assert bool(after.loads) is not reused


@pytest.mark.parametrize("far", [False, True])
def test_same_object_leaf_is_promoted_across_equivalent_pointer_values(far: bool) -> None:
    """Two pointers proved to name one struct field still caused a reload.

    SROA identity is the object and byte range, not the SSA expression used
    to reach it.  A store through one equivalent pointer must feed a later
    load through the other without weakening partial-overlap invalidation.
    """
    from qbopt.model import ir
    from qbopt.model import memory
    from qbopt.objectfile.module import Addr
    from qbopt.objectfile.module import Space

    object_ = memory.Object(
        memory.Kind.NAMED if far else memory.Kind.FRAME,
        7 if far else ("aggregate", -16),
        extent=12,
    )
    leaf = memory.Provenance.one(object_, 4, 8)
    first = mir.Value(1, 0, variable=1, version=1)
    second = mir.Value(2, 2, variable=2, version=1)
    stored = mir.Value(3, 4, variable=3, version=1)
    loaded = mir.Value(4, 6, variable=4, version=1)
    first_segment = mir.Value(5, 0, variable=5, version=1) if far else None
    second_segment = mir.Value(6, 2, variable=6, version=1) if far else None
    space = Space.FAR if far else Space.FRAME
    address = Addr(space, 4 if far else -12, 7 if far else 0)
    via_first = mir.MemRef(
        address,
        4,
        base=first,
        segment=first_segment,
        space=space,
        provenance=leaf,
    )
    via_second = replace(via_first, base=second, segment=second_segment)
    store = mir.Op(
        4,
        ir.Operation.MOVE,
        "mov",
        (),
        tuple(value for value in (stored, first, first_segment) if value is not None),
        kind=mir.Kind.STORE,
        args=(mir.Held(stored, 4),),
        results=(mir.Cell(via_first),),
        stores=(via_first,),
        id=100,
    )
    load = mir.Op(
        6,
        ir.Operation.MOVE,
        "mov",
        (loaded,),
        tuple(value for value in (second, second_segment) if value is not None),
        kind=mir.Kind.LOAD,
        args=(mir.Cell(via_second),),
        results=(mir.Held(loaded, 4),),
        loads=(via_second,),
        id=101,
    )
    body = mir.MirBody(0, (mir.MirBlock(0, (), (store, load), ()),))

    result = promote.promoted(body)
    after = next(op for op in result.blocks[0].ops if op.at == load.at)
    assert not after.loads
    assert all(not isinstance(arg, mir.Cell) for arg in after.args)
    scalarized = promote.promoted(body, aggregate_only=True)
    assert not next(op for op in scalarized.blocks[0].ops if op.at == load.at).loads

    scalar_object = memory.Object(memory.Kind.FRAME, ("scalar", -2), extent=2)
    scalar_ref = mir.MemRef(
        Addr(Space.FRAME, -2),
        2,
        space=Space.FRAME,
        provenance=memory.Provenance.one(scalar_object, 0, 2),
    )
    flags = mir.Value(9, 8, flags=True, variable=9, version=1)
    scalar_update = mir.Op(
        8,
        ir.Operation.UNARY,
        "inc",
        (flags,),
        (),
        kind=mir.Kind.INCREMENT,
        args=(mir.Cell(scalar_ref),),
        results=(mir.Cell(scalar_ref),),
        loads=(scalar_ref,),
        stores=(scalar_ref,),
        id=102,
    )
    aggregate_flags = mir.Value(10, 5, flags=True, variable=10, version=1)
    aggregate_update = mir.Op(
        5,
        ir.Operation.BINARY,
        "add",
        (aggregate_flags,),
        tuple(value for value in (second, second_segment) if value is not None),
        kind=mir.Kind.ADD,
        args=(mir.Cell(via_second), mir.Const(1, 4)),
        results=(mir.Cell(via_second),),
        loads=(via_second,),
        stores=(via_second,),
        id=104,
    )
    mixed = replace(
        body,
        blocks=(replace(body.blocks[0], ops=(store, aggregate_update, load, scalar_update)),),
    )
    scalarized = promote.promoted(mixed, aggregate_only=True)
    aggregate_ops = [op for op in scalarized.blocks[0].ops if op.at == aggregate_update.at]
    assert [op.kind for op in aggregate_ops[:2]] == [mir.Kind.ADD, mir.Kind.STORE]
    assert not aggregate_ops[0].loads and not aggregate_ops[0].stores
    assert set(value for value in (second, second_segment) if value is not None) <= set(aggregate_ops[1].uses)
    untouched = [op for op in scalarized.blocks[0].ops if op.at == scalar_update.at]
    assert untouched == [scalar_update], "early SROA must not split an unrelated scalar memory update"

    integer = replace(via_first, typed=("int4", True))
    pun = replace(via_second, typed=("float4", False))
    typed_store = replace(store, stores=(integer,), results=(mir.Cell(integer),))
    typed_load = replace(load, loads=(pun,), args=(mir.Cell(pun),))
    typed = replace(body, blocks=(replace(body.blocks[0], ops=(typed_store, typed_load)),))
    result = promote.promoted(typed, aggregate_only=True)
    after = next(op for op in result.blocks[0].ops if op.at == typed_load.at)
    assert after.loads == (pun,), "incompatible union member types must keep the object in memory"

    upper_half = mir.MemRef(
        Addr(space, 6 if far else -10, 7 if far else 0),
        2,
        space=space,
        provenance=memory.Provenance.one(object_, 6, 8),
    )
    overwrite = mir.Op(
        6,
        ir.Operation.MOVE,
        "mov",
        (),
        (),
        kind=mir.Kind.STORE,
        args=(mir.Const(0, 2),),
        results=(mir.Cell(upper_half),),
        stores=(upper_half,),
        id=103,
    )
    early_value = mir.Value(7, 5, variable=7, version=1)
    early = replace(
        load,
        at=5,
        defines=(early_value,),
        results=(mir.Held(early_value, 4),),
    )
    late_value = mir.Value(8, 7, variable=8, version=1)
    late = replace(
        load,
        at=7,
        defines=(late_value,),
        results=(mir.Held(late_value, 4),),
    )
    clobbered = replace(body, blocks=(replace(body.blocks[0], ops=(store, early, overwrite, late)),))
    result = promote.promoted(clobbered, aggregate_only=True)
    after = {op.at: op for op in result.blocks[0].ops}
    assert after[early.at].loads == (via_second,), "a partially overlapping object must not be scalarized"
    assert after[late.at].loads == (via_second,), "an overlapping partial store must invalidate the scalar leaf"


def test_sroa_uses_a_singleton_index_range_as_an_exact_leaf() -> None:
    """Two constant-derived indexes into one local array still reloaded it.

    The pointers are different SSA values, but range analysis proves both
    select bytes 4..8 of the same bounded object.  A non-singleton or
    out-of-bounds interval must not receive that exact leaf identity.
    """
    from qbopt.model import ir
    from qbopt.model import memory
    from qbopt.analysis import ranges
    from qbopt.model.passes import Where
    from qbopt.objectfile.module import Addr
    from qbopt.objectfile.module import Space

    object_ = memory.Object(memory.Kind.FRAME, ("array", -16), extent=12)
    whole = memory.Provenance.one(object_)
    first = mir.Value(1, 0, variable=1, version=1)
    second = mir.Value(2, 1, variable=2, version=1)
    stored = mir.Value(3, 2, variable=3, version=1)
    loaded = mir.Value(4, 3, variable=4, version=1)

    def constant(at: int, value: mir.Value) -> mir.Op:
        return mir.Op(
            at,
            ir.Operation.MOVE,
            "mov",
            (value,),
            (),
            kind=mir.Kind.COPY,
            args=(mir.Const(4, 2),),
            results=(mir.Held(value, 2),),
        )

    one = mir.MemRef(Addr(Space.FRAME, 0), 4, base=first, space=Space.FRAME, base_width=2, provenance=whole)
    two = replace(one, base=second)
    store = mir.Op(
        2,
        ir.Operation.MOVE,
        "mov",
        (),
        (stored, first),
        kind=mir.Kind.STORE,
        args=(mir.Held(stored, 4),),
        results=(mir.Cell(one),),
        stores=(one,),
    )
    load = mir.Op(
        3,
        ir.Operation.MOVE,
        "mov",
        (loaded,),
        (second,),
        kind=mir.Kind.LOAD,
        args=(mir.Cell(two),),
        results=(mir.Held(loaded, 4),),
        loads=(two,),
    )
    body = mir.MirBody(0, (mir.MirBlock(0, (), (constant(0, first), constant(1, second), store, load), ()),))

    result = promote.Sroa(Where()).transform(body)
    after = next(op for op in result.blocks[0].ops if op.at == load.at)
    assert not after.loads
    assert all(not isinstance(arg, mir.Cell) for arg in after.args)
    assert promote._bounded_ref(one, {first: ranges.Interval(4, 5, 2)}).provenance == whole
    assert promote._bounded_ref(one, {first: ranges.Interval(12, 12, 2)}).provenance == whole


def test_sroa_uses_exact_frame_pointer_provenance_as_a_leaf() -> None:
    """Matmul retained 64 products after peeling made every array offset constant.

    The cloned initializer reaches each local matrix field through a
    ``FrameAddress + constant`` pointer chain.  Those addresses are not
    integer constants, but pointer analysis proves an exact byte position in
    one bounded frame object; stores and loads through equivalent chains must
    therefore become the same scalar leaf.
    """
    from qbopt.model import ir
    from qbopt.model import memory
    from qbopt.model.passes import Where
    from qbopt.objectfile.module import Addr
    from qbopt.objectfile.module import Space

    root = mir.Value(1, 0, variable=1, version=1)
    first_base = mir.Value(2, 1, variable=2, version=1)
    first = mir.Value(3, 2, variable=3, version=1)
    second_base = mir.Value(4, 3, variable=4, version=1)
    second = mir.Value(5, 4, variable=5, version=1)
    loaded = mir.Value(6, 7, variable=6, version=1)
    object_ = memory.Object(memory.Kind.FRAME, (7, -16, -4), extent=12)
    whole = memory.Provenance.one(object_)

    address = mir.Op(
        0,
        ir.Operation.ADDRESS,
        "lea",
        (root,),
        (),
        kind=mir.Kind.ADDRESS,
        args=(mir.FrameAddress(-16, 2, (-16, -4)),),
        results=(mir.Held(root, 2),),
    )

    def offset(at: int, source: mir.Value, amount: int, result: mir.Value) -> mir.Op:
        return mir.Op(
            at,
            ir.Operation.BINARY,
            "add",
            (result,),
            (source,),
            kind=mir.Kind.ADD,
            args=(mir.Held(source, 2), mir.Const(amount, 2)),
            results=(mir.Held(result, 2),),
        )

    reference = mir.MemRef(
        Addr(Space.LITERAL, 0),
        4,
        base=first,
        space=Space.FRAME,
        base_width=2,
        provenance=whole,
    )
    equivalent = replace(reference, base=second)
    store = mir.Op(
        5,
        ir.Operation.MOVE,
        "mov",
        (),
        (first,),
        kind=mir.Kind.STORE,
        args=(mir.Const(37, 4),),
        results=(mir.Cell(reference),),
        stores=(reference,),
    )
    load = mir.Op(
        6,
        ir.Operation.MOVE,
        "mov",
        (loaded,),
        (second,),
        kind=mir.Kind.LOAD,
        args=(mir.Cell(equivalent),),
        results=(mir.Held(loaded, 4),),
        loads=(equivalent,),
    )
    body = mir.MirBody(
        0,
        (
            mir.MirBlock(
                0,
                (),
                (
                    address,
                    offset(1, root, 16, first_base),
                    offset(2, first_base, 65520, first),
                    offset(3, root, 16, second_base),
                    offset(4, second_base, 65520, second),
                    store,
                    load,
                ),
                (),
            ),
        ),
        pointer_values=frozenset({root}),
        pointer_seeds={root: memory.Provenance.one(object_, 0, 1)},
    )

    result = promote.Sroa(Where()).transform(body)

    after = next(op for op in result.blocks[0].ops if op.at == load.at)
    assert not after.loads
    assert all(not isinstance(arg, mir.Cell) for arg in after.args)


def test_sroa_refines_a_conservative_aggregate_range_from_the_exact_pointer() -> None:
    """PARITY kept every ``points[i].y`` reload after exact unrolling.

    The frontend correctly described the original indexed field as the broad
    byte lane 2..34.  After unrolling, pointer analysis proved each cloned
    access selected one exact address, but SROA accepted that proof only when
    the older annotation covered the *whole* object.  A conservative subrange
    must not veto a more precise same-object pointer fact.
    """
    from qbopt.model import ir
    from qbopt.model import memory
    from qbopt.model.passes import Where
    from qbopt.objectfile.module import Addr
    from qbopt.objectfile.module import Space

    object_ = memory.Object(memory.Kind.FRAME, ("points", -36), extent=32)
    broad = memory.Provenance.one(object_, 2, 34, stride=1, width=2)
    root = mir.Value(1, 0, variable=1, version=1)
    first = mir.Value(2, 1, variable=2, version=1)
    second = mir.Value(3, 2, variable=3, version=1)
    loaded = mir.Value(4, 4, variable=4, version=1)
    address = mir.Op(
        0,
        ir.Operation.ADDRESS,
        "lea",
        (root,),
        (),
        kind=mir.Kind.ADDRESS,
        args=(mir.FrameAddress(-36, 2, (-36, -4)),),
        results=(mir.Held(root, 2),),
    )

    def offset(at: int, result: mir.Value) -> mir.Op:
        return mir.Op(
            at,
            ir.Operation.BINARY,
            "add",
            (result,),
            (root,),
            kind=mir.Kind.ADD,
            args=(mir.Held(root, 2), mir.Const(6, 2)),
            results=(mir.Held(result, 2),),
        )

    ref = mir.MemRef(
        Addr(Space.LITERAL, 0),
        2,
        base=first,
        space=Space.FRAME,
        base_width=2,
        typed=("int2", False),
        provenance=broad,
    )
    equivalent = replace(ref, base=second)
    store = mir.Op(
        3,
        ir.Operation.MOVE,
        "mov",
        (),
        (first,),
        kind=mir.Kind.STORE,
        args=(mir.Const(29, 2),),
        results=(mir.Cell(ref),),
        stores=(ref,),
    )
    load = mir.Op(
        4,
        ir.Operation.MOVE,
        "mov",
        (loaded,),
        (second,),
        kind=mir.Kind.LOAD,
        args=(mir.Cell(equivalent),),
        results=(mir.Held(loaded, 2),),
        loads=(equivalent,),
    )
    body = mir.MirBody(
        0,
        (mir.MirBlock(0, (), (address, offset(1, first), offset(2, second), store, load), ()),),
        pointer_values=frozenset({root}),
        pointer_seeds={root: memory.Provenance.one(object_, 0, 1)},
    )

    result = promote.Sroa(Where()).transform(body)

    after = next(op for op in result.blocks[0].ops if op.at == load.at)
    assert not after.loads
    assert all(not isinstance(arg, mir.Cell) for arg in after.args)


def test_sroa_matches_equivalent_affine_addresses_inside_one_dynamic_allocation() -> None:
    """BASIC PARITY reloaded all 16 fields after its loops were unrolled.

    Stores used a 16-bit ``descriptor_base + offset`` expression while the
    loads used an equivalent zero-extended and reassociated expression.  The
    allocation proof supplies object identity; scalar replacement must use the
    normalized object-relative offset instead of the physical address SSA id.
    """
    from qbopt.model import ir
    from qbopt.model.passes import Where
    from qbopt.objectfile.module import Addr
    from qbopt.objectfile.module import Space

    descriptor = mir.Symbol(Space.FRAME, 0, -38, 2)
    root_cell = mir.MemRef(Addr(Space.FRAME, -28), 2, space=Space.FRAME)
    narrow = mir.Value(1, 1, variable=1, version=1)
    displaced = mir.Value(2, 2, variable=2, version=1)
    extended = mir.Value(3, 3, variable=3, version=1)
    advanced = mir.Value(4, 4, variable=4, version=1)
    equivalent = mir.Value(5, 5, variable=5, version=1)
    loaded = mir.Value(6, 8, variable=6, version=1)
    allocated = mir.Op(
        0,
        ir.Operation.CALL,
        "call",
        (),
        (),
        kind=mir.Kind.CALL,
        array=mir.ArrayRequest(descriptor, 4, ((0, 7),)),
    )

    def binary(at: int, kind: mir.Kind, left: mir.Arg, right: mir.Arg, result: mir.Value, width: int) -> mir.Op:
        return mir.Op(
            at,
            ir.Operation.BINARY,
            kind.value,
            (result,),
            tuple(arg.value for arg in (left, right) if isinstance(arg, mir.Held)),
            kind=kind,
            args=(left, right),
            results=(mir.Held(result, width),),
        )

    first = binary(1, mir.Kind.ADD, mir.Cell(root_cell), mir.Const(0, 2), narrow, 2)
    second = binary(2, mir.Kind.ADD, mir.Cell(root_cell), mir.Const(2, 2), displaced, 2)
    widen = mir.Op(
        3,
        ir.Operation.UNARY,
        "movzx",
        (extended,),
        (displaced,),
        kind=mir.Kind.ZERO_EXTEND,
        args=(mir.Held(displaced, 2),),
        results=(mir.Held(extended, 4),),
    )
    add = binary(4, mir.Kind.ADD, mir.Held(extended, 4), mir.Const(32, 4), advanced, 4)
    cancel = binary(5, mir.Kind.ADD, mir.Held(advanced, 4), mir.Const(0xFFFFFFDE, 4), equivalent, 4)
    stored_ref = mir.MemRef(
        Addr(Space.FAR, 0),
        2,
        base=narrow,
        space=Space.FAR,
        allocation=descriptor,
        base_width=2,
    )
    loaded_ref = replace(stored_ref, base=equivalent, base_width=4)
    store = mir.Op(
        7,
        ir.Operation.MOVE,
        "mov",
        (),
        (narrow,),
        kind=mir.Kind.STORE,
        args=(mir.Const(29, 2),),
        results=(mir.Cell(stored_ref),),
        stores=(stored_ref,),
    )
    load = mir.Op(
        8,
        ir.Operation.MOVE,
        "mov",
        (loaded,),
        (equivalent,),
        kind=mir.Kind.LOAD,
        args=(mir.Cell(loaded_ref),),
        results=(mir.Held(loaded, 2),),
        loads=(loaded_ref,),
    )
    body = mir.MirBody(
        0,
        (mir.MirBlock(0, (), (allocated, first, second, widen, add, cancel, store, load), ()),),
    )

    result = promote.Sroa(Where()).transform(body)

    after = next(op for op in result.blocks[0].ops if op.at == load.at)
    assert not after.loads
    assert all(not isinstance(arg, mir.Cell) for arg in after.args)


def test_sroa_does_not_treat_sign_extension_as_address_preserving() -> None:
    """A sign-extended 16-bit offset is not the same 386 address as its zero extension."""
    from qbopt.model import ir
    from qbopt.objectfile.module import Addr
    from qbopt.objectfile.module import Space

    cell = mir.MemRef(Addr(Space.FRAME, -4), 2, space=Space.FRAME)
    narrow = mir.Value(1, 1, variable=1, version=1)
    wide = mir.Value(2, 2, variable=2, version=1)
    load = mir.Op(
        1,
        ir.Operation.MOVE,
        "mov",
        (narrow,),
        (),
        kind=mir.Kind.LOAD,
        args=(mir.Cell(cell),),
        results=(mir.Held(narrow, 2),),
        loads=(cell,),
    )
    extend = mir.Op(
        2,
        ir.Operation.UNARY,
        "movsx",
        (wide,),
        (narrow,),
        kind=mir.Kind.SIGN_EXTEND,
        args=(mir.Held(narrow, 2),),
        results=(mir.Held(wide, 4),),
    )
    body = mir.MirBody(0, (mir.MirBlock(0, (), (load, extend), ()),))

    assert wide not in promote._affine_values(body)


def test_sroa_never_promotes_a_volatile_aggregate_leaf() -> None:
    """A volatile struct field store followed by a load lost the load.

    Volatility belongs to the memory occurrence, so SROA must reject it even
    if an operation is cloned without the frontend's additional ordering
    barrier. This guards the semantic property independently of C parsing.
    """
    from qbopt.model import ir
    from qbopt.model import memory
    from qbopt.model.passes import Where
    from qbopt.objectfile.module import Addr
    from qbopt.objectfile.module import Space

    object_ = memory.Object(memory.Kind.FRAME, ("volatile aggregate", -8), extent=8)
    ref = mir.MemRef(
        Addr(Space.FRAME, -8),
        4,
        space=Space.FRAME,
        provenance=memory.Provenance.one(object_, 0, 4),
        volatile=True,
    )
    loaded = mir.Value(1, 1, variable=1, version=1)
    store = mir.Op(
        0,
        ir.Operation.MOVE,
        "mov",
        (),
        (),
        kind=mir.Kind.STORE,
        args=(mir.Const(7, 4),),
        results=(mir.Cell(ref),),
        stores=(ref,),
    )
    load = mir.Op(
        1,
        ir.Operation.MOVE,
        "mov",
        (loaded,),
        (),
        kind=mir.Kind.LOAD,
        args=(mir.Cell(ref),),
        results=(mir.Held(loaded, 4),),
        loads=(ref,),
    )
    body = mir.MirBody(0, (mir.MirBlock(0, (), (store, load), ()),))

    result = promote.Sroa(Where()).transform(body)

    assert result == body


@pytest.mark.parametrize("effect", ["call", "barrier"])
def test_partial_store_does_not_restore_constants_from_before_unknown_effect(effect):
    """A post-clobber low-word store must not resurrect an old high word as a promoted LONG."""
    from qbopt.model import ir
    from qbopt.objectfile.module import Addr
    from qbopt.objectfile.module import Space

    cell = mir.MemRef(Addr(Space.FRAME, -8), 4)
    word = replace(cell, width=2)
    value = mir.Value(1, 6, variable=1, version=1)
    initial = mir.Op(
        0,
        ir.Operation.MOVE,
        "",
        (),
        (),
        kind=mir.Kind.STORE,
        args=(mir.Const(0x11223344, 4),),
        results=(mir.Cell(cell),),
        stores=(cell,),
    )
    clobber = mir.Op(
        2,
        ir.Operation.CALL if effect == "call" else ir.Operation.BARRIER,
        "",
        (),
        (),
        kind=mir.Kind.CALL if effect == "call" else mir.Kind.OPAQUE,
    )
    partial = mir.Op(
        4,
        ir.Operation.MOVE,
        "",
        (),
        (),
        kind=mir.Kind.STORE,
        args=(mir.Const(7, 2),),
        results=(mir.Cell(word),),
        stores=(word,),
    )
    load = mir.Op(
        6,
        ir.Operation.MOVE,
        "",
        (value,),
        (),
        kind=mir.Kind.LOAD,
        args=(mir.Cell(cell),),
        results=(mir.Held(value, 4),),
        loads=(cell,),
    )
    body = mir.MirBody(0, (mir.MirBlock(0, (), (initial, clobber, partial, load), ()),))
    result = promote.promoted(body)
    assert next(op for op in result.blocks[0].ops if op.at == 6).loads == (cell,)


def test_unpromotable_memory_update_does_not_cancel_other_cells() -> None:
    """SEGLD rose from 25002 to 28202 when its memory sum canceled counter promotion."""
    from qbopt.optimize import transform

    path = Path("fixtures/omf/segld-p-g2.obj")
    found = module.of(omf.parse(path.read_bytes()))
    partition = blocks.partition(found, blocks.code_map(found))
    body = mir.bodies(found, partition)[0][1]
    counter = next(op.loads[0] for block in body.blocks for op in block.ops if op.at == 0x61)
    body = transform.applied(body, found.dgroup, found.calls, blocks=partition, found=found)
    assert not any(counter in op.loads for block in body.blocks for op in block.ops)


def test_nested_memory_update_becomes_a_value_and_preserves_its_store(monkeypatch: pytest.MonkeyPatch) -> None:
    """NESTED's accumulator stayed a memory ADD instead of a loop-carried value."""
    from qbopt.optimize import transform
    from qbopt.optimize import loopmotion

    monkeypatch.setattr(loopmotion, "sunk_stores", lambda body, *args: body)

    found = module.of(omf.parse(Path("fixtures/omf/nested-p-g2.obj").read_bytes()))
    partition = blocks.partition(found, blocks.code_map(found))
    body = mir.bodies(found, partition)[0][1]
    before = next(op for block in body.blocks for op in block.ops if op.at == 0x7E)
    separated = promote._separated(body)
    computation, write = [op for block in separated.blocks for op in block.ops if op.at == 0x7E]
    assert tuple(value for value in computation.defines if value.flags) == before.defines
    assert write.symbol is True and write.id == before.id
    assert computation.symbol is False
    body = transform.applied(body, found.dgroup, found.calls, blocks=partition, found=found)
    updates = [op for block in body.blocks for op in block.ops if op.at == 0x7E]
    addition = next(op for op in updates if op.kind is mir.Kind.ADD)
    store = next(op for op in updates if op.kind is mir.Kind.STORE)
    assert not addition.loads and not addition.stores
    assert all(isinstance(arg, mir.Held) for arg in addition.args)
    assert store.stores == before.stores
    assert store.args == addition.results


def test_promotion_preserves_existing_cse_value_edges() -> None:
    """flags printed BOTH=nonzero for zero after promotion rebound CSE's constant to an entry phi."""
    from qbopt.optimize import transform

    found = module.of(omf.parse(Path("fixtures/omf/flags-p-g2.obj").read_bytes()))
    partition = blocks.partition(found, blocks.code_map(found))
    body = mir.bodies(found, partition)[0][1]
    body = transform.applied(
        body, found.dgroup, found.calls, blocks=partition, found=found, options=Options(promote=False)
    )
    stored = next(op for block in body.blocks for op in block.ops if op.at == 0x122)
    result = promote.promoted(body, found.dgroup, module.landmarks(found))
    after = next(op for block in result.blocks for op in block.ops if op.id == stored.id)
    assert after.args == stored.args


def test_hotlop_keeps_initialization_for_memory_arithmetic() -> None:
    """hotlop's multiply at 0x4b still reads the cell initialized to 7.

    Promotion removed that initialization but left the multiply in memory,
    so the loop consumed the old memory contents instead of 7.
    """
    found = module.of(omf.parse(Path("fixtures/omf/hotlop-p-g2.obj").read_bytes()))
    mapped = blocks.code_map(found)
    body = mir.bodies(found, blocks.partition(found, mapped))[0][1]
    multiply = next(op for block in body.blocks for op in block.ops if op.at == 0x4B)
    cell = multiply.loads[0]
    from qbopt.analysis import consts

    stores = [op for block in body.blocks for op in block.ops if consts.initialized(op, cell) == consts.Known(7, 2)]
    assert stores
    result = promote.promoted(body, found.dgroup, module.landmarks(found))
    remaining = [op for block in result.blocks for op in block.ops]
    assert all(before in remaining for before in stores)


def test_hotlop_multiply_uses_the_initialized_value() -> None:
    """hotlop's multiply reread A=7 from memory on every iteration."""
    found = module.of(omf.parse(Path("fixtures/omf/hotlop-p-g2.obj").read_bytes()))
    body = mir.bodies(found, blocks.partition(found, blocks.code_map(found)))[0][1]
    result = promote.promoted(body, found.dgroup, module.landmarks(found))
    multiply = next(op for block in result.blocks for op in block.ops if op.at == 0x4B)
    assert multiply.kind is mir.Kind.MUL
    assert not multiply.loads
    assert all(not isinstance(arg, mir.Cell) for arg in multiply.args)


def test_a_read_before_assignment_keeps_its_memory_value() -> None:
    """A load arriving before the first store must not become an undefined SSA input."""
    found = module.of(omf.parse(Path("fixtures/omf/press-p-g2.obj").read_bytes()))
    body = mir.bodies(found, blocks.partition(found, blocks.code_map(found)))[0][1]
    ops = [op for block in body.blocks for op in block.ops]
    load = next(op for op in ops if op.at == 0x94)
    store = next(op for op in ops if op.at == 0x98)
    body = replace(body, entry=0, blocks=(mir.MirBlock(0, (), (load, store), ()),))
    result = promote.promoted(body, found.dgroup, module.landmarks(found))
    assert result.blocks[0].ops[0].loads == load.loads


def test_promoted_global_remains_visible_outside_the_body() -> None:
    """press's loop counter is global: forwarding its load cannot delete its stores."""
    found = module.of(omf.parse(Path("fixtures/omf/press-p-g2.obj").read_bytes()))
    body = mir.bodies(found, blocks.partition(found, blocks.code_map(found)))[0][1]
    result = promote.promoted(body, found.dgroup, module.landmarks(found))
    before = [ref for block in body.blocks for op in block.ops for ref in op.stores]
    after = [ref for block in result.blocks for op in block.ops for ref in op.stores]
    assert before == after
    load = next(op for block in result.blocks for op in block.ops if op.at == 0x94)
    assert not load.loads, "the loop should use the value stored in its header"


def test_production_press_keeps_the_loop_counter_in_a_value() -> None:
    """press reloaded J each iteration despite having just stored that value."""
    raw = Path("fixtures/omf/press-p-g2.obj").read_bytes()
    found = module.of(omf.parse(raw))
    body = mir.bodies(found, blocks.partition(found, blocks.code_map(found)))[0][1]
    cell = next(op.loads[0] for block in body.blocks for op in block.ops if op.at == 0x94)
    result = wholeseg.emitted(raw)
    assert result.outcome is wholeseg.Emission.LIR, result
    emitted = module.of(omf.parse(result.data))
    bodies = mir.bodies(emitted, blocks.partition(emitted, blocks.code_map(emitted)))
    refs = [ref for _, body in bodies for block in body.blocks for op in block.ops for ref in op.loads]
    assert not any(ref.addr == cell.addr for ref in refs), "the emitted loop still reloads J"


@pytest.mark.parametrize(("position", "reused"), [(0, True), (1, False), (2, True)])
@pytest.mark.parametrize("effect", ["explicit", "unspecified", "barrier"])
def test_only_an_intervening_call_invalidates_a_stored_value(position: int, reused: bool, effect: str) -> None:
    """A later call cannot invalidate an earlier read; an intervening call must."""
    found = module.of(omf.parse(Path("fixtures/omf/press-p-g2.obj").read_bytes()))
    body = mir.bodies(found, blocks.partition(found, blocks.code_map(found)))[0][1]
    ops = [op for block in body.blocks for op in block.ops]
    load = next(op for op in ops if op.at == 0x94)
    store = next(op for op in ops if op.at == 0x98)
    call = replace(ops[-1], stores=(mir.MemRef(None, 0),))
    if effect != "explicit":
        call = replace(call, stores=())
    if effect == "barrier":
        from qbopt.model import ir

        call = replace(call, op=ir.Operation.BARRIER, kind=mir.Kind.OPAQUE)
    sequence = [store, load]
    sequence.insert(position, call)
    body = replace(body, entry=0, blocks=(mir.MirBlock(0, (), tuple(sequence), ()),))
    result = promote.promoted(body, found.dgroup, module.landmarks(found))
    load = next(op for op in result.blocks[0].ops if op.at == load.at)
    assert bool(load.loads) is not reused


@pytest.mark.parametrize("tag", ["p-g2", "q-O", "v-g3"])
def test_spill_accumulator_is_a_loop_carried_value(tag):
    """SPILL's packed zero initializer prevented promotion of t across its hundred inner iterations."""
    from qbopt.analysis import loops
    from qbopt.optimize import transform
    path = Path(f"fixtures/omf/spill-{tag}.obj".lower())
    found = module.of(omf.parse(path.read_bytes()))
    partition = blocks.partition(found, blocks.code_map(found))
    body = mir.bodies(found, partition)[0][1]
    result = transform.applied(body, found.dgroup, found.calls, blocks=partition, found=found)
    inner = {
        at
        for loop in loops.loops(result.blocks, result.entry)
        if not any(other.body < loop.body for other in loops.loops(result.blocks, result.entry))
        for at in loop.body
    }
    assert not any(op.loads or op.stores for block in result.blocks if block.at in inner for op in block.ops)


def test_packed_capture_keeps_wide_and_narrow_definitions_and_rejects_unknown_overlap():
    """Capturing one field must not lose the whole store or reuse a field after an unknown wide write."""
    from qbopt.model import ir
    from qbopt.objectfile.module import Addr
    from qbopt.objectfile.module import Space

    address = Addr(Space.SEGMENT, 6, 5)
    whole = mir.MemRef(address, 4)
    half = mir.MemRef(address.plus(2), 2)
    first = mir.Op(
        0, ir.Operation.MOVE, "mov", (), (), kind=mir.Kind.STORE, args=(mir.Const(0x12345678, 4),), stores=(whole,)
    )

    def load(at, ref):
        result = mir.Value(at, at, variable=at, version=1)
        return mir.Op(
            at,
            ir.Operation.MOVE,
            "mov",
            (result,),
            (),
            kind=mir.Kind.LOAD,
            args=(mir.Cell(ref),),
            results=(mir.Held(result, ref.width),),
            loads=(ref,),
        )

    incoming = mir.Value(100, 0, variable=100, version=1)
    overwrite = mir.Op(
        14,
        ir.Operation.MOVE,
        "mov",
        (),
        (incoming,),
        kind=mir.Kind.STORE,
        args=(mir.Held(incoming, 4),),
        stores=(whole,),
    )
    body = mir.MirBody(
        0, (mir.MirBlock(0, (), (first, load(8, whole), load(10, half), overwrite, load(18, half)), ()),)
    )
    result = promote.promoted(body, frozenset({5}))
    ops = result.blocks[0].ops
    assert first in ops and overwrite in ops
    assert not next(op for op in ops if op.at == 8).loads
    assert not next(op for op in ops if op.at == 10).loads
    assert next(op for op in ops if op.at == 18).loads == (half,)
    captures = [op for op in ops if op.at == 0 and op.kind is mir.Kind.COPY]
    assert {op.results[0].width for op in captures} == {2, 4}


@pytest.mark.parametrize("tag", ["p-g2", "q-O", "v-g3"])
def test_addrm_long_accumulator_survives_split_initialization(tag):
    """ADDRM reloaded u on all 20 iterations despite initializing both words to zero."""
    from qbopt.analysis import loops
    from qbopt.optimize import transform
    found = module.of(omf.parse(Path(f"fixtures/omf/addrm-{tag}.obj".lower()).read_bytes()))
    partition = blocks.partition(found, blocks.code_map(found))
    body = mir.bodies(found, partition)[0][1]
    cell = next(
        ref for block in body.blocks for op in block.ops for ref in op.loads if ref.width == 4 and ref.base is None
    )
    output = [
        op.id
        for block in body.blocks
        for op in block.ops
        if op.kind is mir.Kind.ARG and any(mir.overlapping(ref, cell, found.dgroup) for ref in op.loads)
    ]
    result = transform.applied(body, found.dgroup, found.calls, blocks=partition, found=found)
    inside = {at for loop in loops.loops(result.blocks, result.entry) for at in loop.body}
    assert not any(cell in op.loads for block in result.blocks if block.at in inside for op in block.ops)
    assert output
    # PRINT still gets u, from the cell or from the value promoted out of it.
    remaining = {op.id for block in result.blocks for op in block.ops if op.kind is mir.Kind.ARG}
    assert set(output) <= remaining


@pytest.mark.parametrize("complete", [False, True])
def test_split_initializer_requires_every_byte(complete):
    """ADDRM's two word stores may initialize a long; one word must not invent the other."""
    from qbopt.model import ir
    from qbopt.objectfile.module import Addr
    from qbopt.objectfile.module import Space

    address = Addr(Space.SEGMENT, 6, 5)
    whole = mir.MemRef(address, 4)

    def store(at, offset, number):
        return mir.Op(
            at,
            ir.Operation.MOVE,
            "mov",
            (),
            (),
            kind=mir.Kind.STORE,
            args=(mir.Const(number, 2),),
            stores=(mir.MemRef(address.plus(offset), 2),),
        )

    value = mir.Value(10, 10, variable=10, version=1)
    load = mir.Op(
        10,
        ir.Operation.MOVE,
        "mov",
        (value,),
        (),
        kind=mir.Kind.LOAD,
        args=(mir.Cell(whole),),
        results=(mir.Held(value, 4),),
        loads=(whole,),
    )
    stores = (store(0, 0, 0x5678), store(2, 2, 0x1234)) if complete else (store(0, 0, 0x5678),)
    body = mir.MirBody(0, (mir.MirBlock(0, (), (*stores, load), ()),))
    result = promote.promoted(body, frozenset({5}))
    ops = result.blocks[0].ops
    assert all(op in ops for op in stores)
    assert bool(next(op for op in ops if op.at == 10).loads) is not complete
    if complete:
        assert any(op.kind is mir.Kind.COPY and op.args == (mir.Const(0x12345678, 4),) for op in ops)
