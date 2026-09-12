from pathlib import Path

import pytest

from tests import corpus
from qbopt.model import ir
from qbopt.model import mir
from qbopt.analysis import frameescape


def test_renderer_exposes_temporary_string_not_counter_address() -> None:
    path = Path("fixtures/regressions/qrender-d-surf-v-g3.obj")
    found = corpus.loaded(path)
    assert found is not None
    body = next(body for name, body in mir.bodies(found, corpus.partitioned(path)) if name.endswith(" LS_ANIMATE"))
    result = frameescape.analysed(body)
    assert result.exposed == frozenset({-32})
    assert not result.opaque_addresses


@pytest.mark.parametrize("sink", [mir.Kind.CALL, mir.Kind.STORE, mir.Kind.RETURN, mir.Kind.ADD])
def test_frame_origin_reaches_use_through_copy_and_loop_phi(sink: mir.Kind) -> None:
    root, joined, copied = (mir.Value(index, index) for index in range(1, 4))
    address = mir.Op(
        0,
        ir.Operation.ADDRESS,
        "",
        (root,),
        (),
        kind=mir.Kind.ADDRESS,
        args=(mir.FrameAddress(-32, 2),),
        results=(mir.Held(root, 2),),
    )
    copy = mir.Op(
        2,
        ir.Operation.MOVE,
        "",
        (copied,),
        (joined,),
        kind=mir.Kind.COPY,
        args=(mir.Held(joined, 2),),
        results=(mir.Held(copied, 2),),
    )
    use = mir.Op(3, ir.Operation.NOTHING, "", (), (copied,), kind=sink, args=(mir.Held(copied, 2),))
    body = mir.MirBody(
        0,
        (
            mir.MirBlock(0, (), (address,), (1,)),
            mir.MirBlock(1, (mir.Phi(joined, {0: root, 1: copied}),), (copy, use), (1,)),
        ),
    )
    result = frameescape.analysed(body)
    assert result.origins[copied] == frozenset({-32})
    assert result.exposed == frozenset({-32})


def test_opaque_address_is_not_an_empty_escape_proof() -> None:
    op = mir.Op(8, ir.Operation.ADDRESS, "", (), (), kind=mir.Kind.ADDRESS, args=(mir.Opaque(None),))
    result = frameescape.analysed(mir.MirBody(8, (mir.MirBlock(8, (), (op,), ()),)))
    assert result.opaque_addresses == frozenset({8})
