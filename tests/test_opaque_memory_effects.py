from pathlib import Path

from tests import corpus
from qbopt.model import mir
from qbopt.frontend import blocks
from qbopt.analysis import effects
from qbopt.objectfile import module
from qbopt.frontend import fppatches


def test_renderer_status_store_does_not_clobber_the_counter() -> None:
    # r_cull_box's status word at BP-4 was treated as a write to every
    # local, including its unrelated loop counter at BP-2.
    found = corpus.loaded(Path("fixtures/regressions/r_walk-borland.obj"))
    assert found is not None
    mapped = blocks.code_map(found)
    assert not isinstance(mapped, str)
    found = module.of(fppatches.native_records(found, mapped.starts))
    assert found is not None
    mapped = blocks.code_map(found)
    assert not isinstance(mapped, str)
    body = next(body for _, body in mir.bodies(found, blocks.partition(found, mapped)) if body.entry == 0)
    operations = {op.at: op for block in body.blocks for op in block.ops}
    counter = operations[0x2E0].loads[0]
    status = operations[0x5D].loads[0]
    write = operations[0x58]
    assert write.barrier, "unmodeled status computation must remain pinned"
    assert not effects.unmodeled_write(write)
    assert not write.loads
    assert len(write.stores) == 1
    assert mir.same_bytes(write.stores[0], status)
    assert not mir.overlapping(write.stores[0], counter, found.dgroup)
    for at in (0x55, 0x60, 0x2D3):
        op = operations[at]
        assert op.barrier
        assert not effects.unmodeled_write(op)
        assert not op.loads and not op.stores
