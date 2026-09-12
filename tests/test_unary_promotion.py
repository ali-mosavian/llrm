from pathlib import Path
from dataclasses import replace

from tests import corpus
from qbopt.model import mir
from qbopt.frontend import blocks
from qbopt.optimize import promote
from qbopt.objectfile import module
from qbopt.frontend import fppatches


def test_real_counter_update_reuses_its_initialized_value() -> None:
    # r_cull_box's INC [bp-2] was not among promotion's supported updates.
    # Isolate its real initialization/update/read to test this independently
    # of the still-opaque far-pointer load elsewhere in the procedure.
    found = corpus.loaded(Path("fixtures/regressions/r_walk-borland.obj"))
    assert found is not None
    mapped = blocks.code_map(found)
    assert not isinstance(mapped, str)
    found = module.of(fppatches.native_records(found, mapped.starts))
    assert found is not None
    mapped = blocks.code_map(found)
    assert not isinstance(mapped, str)
    body = next(body for _, body in mir.bodies(found, blocks.partition(found, mapped)) if body.entry == 0)
    sequence = tuple(op for block in body.blocks for op in block.ops if op.at in (0xB, 0x2E0, 0x2E3))
    original = next(op for op in sequence if op.at == 0x2E0)
    isolated = replace(body, blocks=(mir.MirBlock(0, (), sequence, ()),))
    result = promote.promoted(isolated, found.dgroup)
    update = next(op for block in result.blocks for op in block.ops if op.kind is mir.Kind.INCREMENT)
    assert not update.loads and not update.stores
    assert len(update.args) == 1 and isinstance(update.args[0], mir.Held)
    assert tuple(value for value in update.defines if value.flags) == original.defines
    stores = [op for block in result.blocks for op in block.ops if op.at == 0x2E0 and op.stores]
    assert len(stores) == 1
    assert stores[0].stores == original.stores
    assert stores[0].args == update.results
