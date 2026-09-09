"""Promotion must replace every access to a cell, or leave it in memory."""

from pathlib import Path
from dataclasses import replace

import pytest

from qbopt import mir
from qbopt import omf
from qbopt import blocks
from qbopt import module
from qbopt import promote
from qbopt import wholeseg


def test_nested_memory_update_becomes_a_value_and_preserves_its_store() -> None:
    """NESTED's accumulator stayed a memory ADD instead of a loop-carried value."""
    from qbopt import transform

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
    from qbopt import transform

    found = module.of(omf.parse(Path("fixtures/omf/flags-p-g2.obj").read_bytes()))
    partition = blocks.partition(found, blocks.code_map(found))
    body = mir.bodies(found, partition)[0][1]
    body = transform.applied(body, found.dgroup, found.calls, blocks=partition, found=found, promote_=False)
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
    stores = [op for block in body.blocks for op in block.ops if cell in op.stores]
    assert stores
    result = promote.promoted(body, found.dgroup, module.landmarks(found))
    remaining = [op for block in result.blocks for op in block.ops]
    assert all(any(op.at == before.at and cell in op.stores for op in remaining) for before in stores)


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
def test_only_an_intervening_call_invalidates_a_stored_value(position: int, reused: bool) -> None:
    """A later call cannot invalidate an earlier read; an intervening call must."""
    found = module.of(omf.parse(Path("fixtures/omf/press-p-g2.obj").read_bytes()))
    body = mir.bodies(found, blocks.partition(found, blocks.code_map(found)))[0][1]
    ops = [op for block in body.blocks for op in block.ops]
    load = next(op for op in ops if op.at == 0x94)
    store = next(op for op in ops if op.at == 0x98)
    call = replace(ops[-1], stores=(mir.MemRef(None, 0),))
    sequence = [store, load]
    sequence.insert(position, call)
    body = replace(body, entry=0, blocks=(mir.MirBlock(0, (), tuple(sequence), ()),))
    result = promote.promoted(body, found.dgroup, module.landmarks(found))
    load = next(op for op in result.blocks[0].ops if op.at == load.at)
    assert bool(load.loads) is not reused
