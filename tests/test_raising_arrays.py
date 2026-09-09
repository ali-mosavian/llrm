from pathlib import Path
from dataclasses import replace

import pytest

import corpus
from qbopt import ir
from qbopt import mir
from qbopt import wholeseg
from qbopt.module import Space
from qbopt import raising_arrays


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
def test_array_annotation_does_not_change_emission(tag: str, monkeypatch: pytest.MonkeyPatch) -> None:
    data = (Path("fixtures/omf") / f"harr-{tag}.obj").read_bytes()
    annotated = wholeseg.emitted(data)
    monkeypatch.setattr(raising_arrays, "annotated", lambda body, calls: body)
    original = wholeseg.emitted(data)
    assert annotated.outcome is wholeseg.Emission.LIR
    assert original.outcome is wholeseg.Emission.LIR
    assert annotated.data == original.data
