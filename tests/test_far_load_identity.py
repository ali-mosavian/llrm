from pathlib import Path

from tests import corpus
from qbopt.model import ir
from qbopt.model import mir
from qbopt.backend import lower
from qbopt.frontend import blocks
from qbopt.analysis import effects
from qbopt.objectfile import module
from qbopt.frontend import fppatches


def test_culling_far_load_defines_the_successors_address_space() -> None:
    # LES invalidated the initialized loop counter, while its successor
    # blocks had no explicit identity for the newly loaded address space.
    found = corpus.loaded(Path("fixtures/regressions/r_walk-borland.obj"))
    assert found is not None
    mapped = blocks.code_map(found)
    assert not isinstance(mapped, str)
    found = module.of(fppatches.native_records(found, mapped.starts))
    assert found is not None
    mapped = blocks.code_map(found)
    assert not isinstance(mapped, str)
    body = next(body for _, body in mir.bodies(found, blocks.partition(found, mapped)) if body.entry == 0)
    pointer = next(op for block in body.blocks for op in block.ops if op.at == 0x1F)
    access = next(op for block in body.blocks for op in block.ops if op.at == 0x22)
    assert access.loads[0].segment is not None
    assert access.loads[0].segment in pointer.defines
    assert access.loads[0].segment in access.uses
    header = next(block for block in body.blocks if block.at == 0x4A)
    assert any(access.loads[0].segment in phi.incoming.values() for phi in header.phis)
    selector = next(op for op in header.ops if op.at == 0x4A)
    plane = next(op for op in header.ops if op.at == 0x4E)
    assert plane.loads[0].segment in selector.defines
    assert plane.loads[0].segment != access.loads[0].segment
    assert pointer.barrier
    assert not effects.unmodeled_write(pointer)
    assert not pointer.stores
    assert len(pointer.loads) == 1 and pointer.loads[0].width == 4


def test_restored_selector_defines_the_following_store_address() -> None:
    # r_walk rendered zero polygons: POP ES defined no LIR value, so the
    # following store reloaded an unwritten spill slot over the restored ES.
    found = corpus.loaded(Path("fixtures/regressions/r_walk-borland.obj"))
    assert found is not None
    mapped = blocks.code_map(found)
    assert not isinstance(mapped, str)
    body = next(body for _, body in mir.bodies(found, blocks.partition(found, mapped)) if body.entry == 0x334)
    restore = next(op for block in body.blocks for op in block.ops if op.at == 0x413)
    store = next(op for block in body.blocks for op in block.ops if op.at == 0x414)
    state = store.stores[0].segment
    assert state is not None and state in restore.defines
    instruction = lower.current(restore, lower.as_a_value)
    assert instruction is not None
    assert instruction.dests == (ir.Held(state.id, 2),)
