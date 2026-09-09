from pathlib import Path
from dataclasses import replace

import pytest

import corpus
from qbopt import ir
from qbopt import mir
from qbopt import wholeseg
from qbopt.module import Space
from qbopt import raising_arrays
from qbopt import consts


@pytest.mark.parametrize("tag", ["p-g2", "q-O", "v-g3"])
def test_dim_normal_return_supplies_descriptor_constants(tag: str) -> None:
    """HARR's 21-element dimensions were unknown immediately after DDIM returned."""
    path = Path("fixtures/omf") / f"harr-{tag}.obj"
    found = corpus.loaded(path)
    body = mir.bodies(found, corpus.partitioned(path))[0][1]
    call = next(op for block in body.blocks for op in block.ops if op.array)
    facts = consts._kills({}, call, {}, found.dgroup, found.calls)
    from qbopt.module import Addr

    assert consts._cell(facts, mir.MemRef(Addr(Space.SEGMENT, 20, found.program_data), 2)) == consts.Known(21, 2)
    assert consts._cell(facts, mir.MemRef(Addr(Space.SEGMENT, 24, found.program_data), 2)) == consts.Known(21, 2)
    unknown = replace(call, kind=mir.Kind.STORE, memory_values=())
    assert consts._kills(facts, unknown, {}, found.dgroup, {}) == {}


def test_descriptor_dimensions_follow_stack_order() -> None:
    """Unequal dimensions must not be swapped: the last pushed bound lives at descriptor +14."""
    request = mir.ArrayRequest(mir.Symbol(Space.SEGMENT, 5, 6, 2), 2, ((-3, 2), (4, 14)))
    arguments = [mir.Const(2, 2), mir.Const(2, 2), request.descriptor]
    fields = raising_arrays._descriptor_values(request, arguments, "qb45")
    assert [(ref.addr.disp, value.n) for ref, value in fields] == [
        (14, 2),
        (18, 2),
        (20, 11),
        (22, 4),
        (24, 6),
        (26, -3),
    ]
    assert raising_arrays._descriptor_values(request, arguments, "unknown") == ()
    arguments[-2] = mir.Const(0x8002, 2)
    assert raising_arrays._descriptor_values(request, arguments, "vbdos") == ()


@pytest.mark.parametrize("tag", ["p-g2", "q-O", "v-g3"])
def test_descriptor_fields_have_proven_addresses_without_new_relocations(tag: str) -> None:
    """HARR's descriptor fields looked like arbitrary pointer accesses to alias analysis."""
    from qbopt.module import Addr

    path = Path("fixtures/omf") / f"harr-{tag}.obj"
    found = corpus.loaded(path)
    body = mir.bodies(found, corpus.partitioned(path))[0][1]
    fields = [ref for block in body.blocks for op in block.ops for ref in op.loads if ref.symbolic is not None]
    assert {ref.symbolic.offset for ref in fields} >= {8, 16}
    for ref in fields:
        assert ref.addr.space is Space.LITERAL and ref.base is not None
        symbol = ref.symbolic
        direct = mir.MemRef(Addr(Space.SEGMENT, symbol.offset, symbol.index), ref.width)
        assert mir.same_bytes(ref, direct)
        assert mir.overlapping(ref, direct, found.dgroup)
        unrelated = replace(direct, addr=Addr(Space.SEGMENT, symbol.offset + ref.width, symbol.index))
        assert not mir.overlapping(ref, unrelated, found.dgroup)


@pytest.mark.parametrize("space,offset,width", [(Space.FAR, 6, 2), (Space.LITERAL, 65535, 2), (Space.LITERAL, 6, 4)])
def test_unknown_segment_wrapping_or_wide_pointer_is_not_resolved(space: Space, offset: int, width: int) -> None:
    from qbopt.module import Addr

    pointer = mir.Value(1, 0)
    ref = mir.MemRef(Addr(space, 2), 2, pointer)
    op = mir.Op(0, ir.Operation.MOVE, "mov", (), (pointer,), loads=(ref,), args=(mir.Cell(ref),))
    body = mir.MirBody(0, (mir.MirBlock(0, (), (op,), ()),))
    result = raising_arrays._addresses(body, {pointer: mir.Symbol(Space.SEGMENT, 5, offset, width)})
    assert result.blocks[0].ops[0].loads[0].symbolic is None


@pytest.mark.parametrize("tag", ["p-g2", "q-O", "v-g3"])
@pytest.mark.parametrize(("name", "bounds"), [("harr", ((0, 20), (0, 20))), ("segld", ((0, 100),))])
def test_real_array_requests(name: str, bounds: tuple[tuple[int, int], ...], tag: str) -> None:
    obj = Path("fixtures/omf") / f"{name}-{tag}.obj"
    found = corpus.loaded(obj)
    assert found is not None
    requests = [
        op.array
        for _, body in mir.bodies(found, corpus.partitioned(obj))
        for block in body.blocks
        for op in block.ops
        if op.array is not None
    ]
    assert len(requests) == 1
    request = requests[0]
    assert request.bounds == bounds
    assert request.element_width == 2
    assert request.descriptor.index == found.program_data
    assert not request.replaces


@pytest.mark.parametrize("name", ["B$DDIM", "B$RDIM", "B$ADIM", "unknown"])
def test_only_allocating_calls_carry_requests(name: str) -> None:
    descriptor = mir.Symbol(Space.SEGMENT, 5, 6, 2)
    args = (mir.Const(-2, 2), mir.Const(3, 2), mir.Const(4, 2), mir.Const(257, 2), descriptor)
    pushes = tuple(
        mir.Op(at, ir.Operation.PUSH, "push", (), (), kind=mir.Kind.ARG, args=(arg,)) for at, arg in enumerate(args)
    )
    call = mir.Op(5, ir.Operation.CALL, "call", (), (), kind=mir.Kind.CALL)
    body = mir.MirBody(0, (mir.MirBlock(0, (), (*pushes, call), ()),))
    result = raising_arrays.annotated(body, {5: name}).blocks[0].ops[-1].array
    if name in ("B$DDIM", "B$RDIM"):
        assert result == mir.ArrayRequest(descriptor, 4, ((-2, 3),), name == "B$RDIM")
    else:
        assert result is None
    interrupted = replace(body, blocks=(replace(body.blocks[0], ops=(*pushes, replace(call, at=4), call)),))
    assert raising_arrays.annotated(interrupted, {4: "unknown", 5: name}).blocks[0].ops[-1].array is None


@pytest.mark.parametrize("tag", ["p-g2", "q-O", "v-g3"])
def test_descriptor_metadata_alone_does_not_change_emission(tag: str, monkeypatch: pytest.MonkeyPatch) -> None:
    from qbopt import raising_array_bounds

    monkeypatch.setattr(raising_array_bounds, "proven", lambda body: body)
    data = (Path("fixtures/omf") / f"harr-{tag}.obj").read_bytes()
    annotated = wholeseg.emitted(data)
    monkeypatch.setattr(raising_arrays, "annotated", lambda body, calls, **kwargs: body)
    original = wholeseg.emitted(data)
    assert annotated.outcome is wholeseg.Emission.LIR
    assert original.outcome is wholeseg.Emission.LIR
    assert annotated.data == original.data
