from pathlib import Path

from tests import corpus
from qbopt.model import mir
from qbopt.backend import asm
from qbopt.backend import lower
from qbopt.frontend import blocks


def test_carried_float_compare_keeps_its_constant_relocation() -> None:
    # d_faces drew 261 polygons instead of 258: FCOMP lost the 0.01 constant.
    found = corpus.loaded(Path("fixtures/regressions/d_faces-borland.obj"))
    assert found is not None
    mapped = blocks.code_map(found)
    assert not isinstance(mapped, str)
    bodies = mir.bodies(found, list(blocks.partition(found, mapped)))
    body = next(body for _, body in bodies if body.entry == 0x38A)
    op = next(op for block in body.blocks for op in block.ops if op.at == 0x566)
    isolated = mir.MirBody(op.at, (mir.MirBlock(op.at, (), (op,), ()),), origin=body.origin)
    lowered = lower.lowered("compare", isolated, {}, {}, {}, nodes=bodies.source.nodes)
    carried = lowered.blocks[0].insns
    laid = asm.assemble(list(carried), 0, found, native_fpu=True)
    assert isinstance(laid, asm.Laid), laid
    assert laid.code == found.code[0x566:0x56A]
    assert laid.relocations == ((2, 0x568),)
    assert 0x568 not in laid.dropped
