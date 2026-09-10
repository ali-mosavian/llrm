"""Write-through promotion reuses proven values while preserving other memory accesses."""

from pathlib import Path
from dataclasses import replace

import pytest

from qbopt.model import mir
from qbopt.objectfile import omf
from qbopt.frontend import blocks
from qbopt.objectfile import module
from qbopt.optimize import promote
from qbopt import wholeseg


def test_procedure_frame_fields_reuse_stored_values():
    """LOCALP reread its frame accumulator on every addition despite known stores."""
    from qbopt.objectfile.module import Space
    path = Path("fixtures/regressions/localp-p-g2.obj")
    found = module.of(omf.parse(path.read_bytes()))
    body = next(body for name, body in mir.bodies(found, blocks.partition(found, blocks.code_map(found)))
                if body.entry != 0x30)
    before = next(op for block in body.blocks for op in block.ops
                  if op.kind is mir.Kind.LOAD and op.loads and op.loads[0].width == 4
                  and op.loads[0].addr.space is Space.FRAME)
    result = promote.promoted(body, found.dgroup, module.landmarks(found), loop_only=True)
    after = next(op for block in result.blocks for op in block.ops if op.id == before.id)
    assert not after.loads
    assert all(not isinstance(arg, mir.Cell) for arg in after.args)


@pytest.mark.parametrize("clobber,reused", [(None, False), (-8, False), (-6, True)])
def test_frame_promotion_respects_unknown_and_overlapping_writes(clobber, reused):
    """LOCALP's held frame field must not survive an unknown call or a write to that field."""
    from qbopt.model import ir
    from qbopt.objectfile.module import Addr, Space
    cell = mir.MemRef(Addr(Space.FRAME, -8), 2)
    changed = mir.MemRef(None, 0) if clobber is None else mir.MemRef(Addr(Space.FRAME, clobber), 2)
    value = mir.Value(1, 4, variable=1, version=1)
    store = mir.Op(0, ir.Operation.MOVE, "", (), (), kind=mir.Kind.STORE,
                   args=(mir.Const(7, 2),), results=(mir.Cell(cell),), stores=(cell,))
    write = mir.Op(2, ir.Operation.CALL, "", (), (), kind=mir.Kind.CALL, stores=(changed,))
    load = mir.Op(4, ir.Operation.MOVE, "", (value,), (), kind=mir.Kind.LOAD,
                  args=(mir.Cell(cell),), results=(mir.Held(value, 2),), loads=(cell,))
    body = mir.MirBody(0, (mir.MirBlock(0, (), (store, write, load), ()),))
    result = promote.promoted(body)
    after = next(op for block in result.blocks for op in block.ops if op.at == 4)
    assert bool(after.loads) is not reused


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
    from qbopt.analysis import consts
    stores = [op for block in body.blocks for op in block.ops
              if consts.initialized(op, cell) == consts.Known(7, 2)]
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

@pytest.mark.parametrize("tag", ["p-g2", "q-O", "v-g3"])
def test_spill_accumulator_is_a_loop_carried_value(tag):
    """SPILL's packed zero initializer prevented promotion of t across its hundred inner iterations."""
    from qbopt.optimize import transform
    from qbopt.analysis import loops
    path = Path(f"fixtures/omf/spill-{tag}.obj")
    found = module.of(omf.parse(path.read_bytes()))
    partition = blocks.partition(found, blocks.code_map(found))
    body = mir.bodies(found, partition)[0][1]
    result = transform.applied(body, found.dgroup, found.calls, blocks=partition, found=found)
    inner = {at for loop in loops.loops(result.blocks, result.entry)
             if not any(other.body < loop.body for other in loops.loops(result.blocks, result.entry))
             for at in loop.body}
    assert not any(op.loads or op.stores for block in result.blocks if block.at in inner for op in block.ops)


def test_packed_capture_keeps_wide_and_narrow_definitions_and_rejects_unknown_overlap():
    """Capturing one field must not lose the whole store or reuse a field after an unknown wide write."""
    from qbopt.model import ir
    from qbopt.objectfile.module import Addr, Space
    address = Addr(Space.SEGMENT, 6, 5)
    whole = mir.MemRef(address, 4)
    half = mir.MemRef(address.plus(2), 2)
    first = mir.Op(0, ir.Operation.MOVE, "mov", (), (), kind=mir.Kind.STORE,
                   args=(mir.Const(0x12345678, 4),), stores=(whole,), covers=(0, 8))
    def load(at, ref):
        result = mir.Value(at, at, variable=at, version=1)
        return mir.Op(at, ir.Operation.MOVE, "mov", (result,), (), kind=mir.Kind.LOAD,
                      args=(mir.Cell(ref),), results=(mir.Held(result, ref.width),), loads=(ref,), covers=(at, at+2))
    incoming = mir.Value(100, 0, variable=100, version=1)
    overwrite = mir.Op(14, ir.Operation.MOVE, "mov", (), (incoming,), kind=mir.Kind.STORE,
                       args=(mir.Held(incoming, 4),), stores=(whole,), covers=(14, 18))
    body = mir.MirBody(0, (mir.MirBlock(0, (), (first, load(8, whole), load(10, half),
                      overwrite, load(18, half)), ()),))
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
    found = module.of(omf.parse(Path(f"fixtures/omf/addrm-{tag}.obj").read_bytes()))
    partition = blocks.partition(found, blocks.code_map(found))
    body = mir.bodies(found, partition)[0][1]
    cell = next(ref for block in body.blocks for op in block.ops for ref in op.loads
                if ref.width == 4 and ref.base is None)
    output = [op.loads for block in body.blocks for op in block.ops
              if op.kind is mir.Kind.ARG and any(mir.overlapping(ref, cell, found.dgroup) for ref in op.loads)]
    result = transform.applied(body, found.dgroup, found.calls, blocks=partition, found=found)
    inside = {at for loop in loops.loops(result.blocks, result.entry) for at in loop.body}
    assert not any(cell in op.loads for block in result.blocks if block.at in inside for op in block.ops)
    assert output
    remaining = [op.loads for block in result.blocks for op in block.ops if op.kind is mir.Kind.ARG]
    assert all(refs in remaining for refs in output)


@pytest.mark.parametrize("complete", [False, True])
def test_split_initializer_requires_every_byte(complete):
    """ADDRM's two word stores may initialize a long; one word must not invent the other."""
    from qbopt.model import ir
    from qbopt.objectfile.module import Addr, Space
    address = Addr(Space.SEGMENT, 6, 5)
    whole = mir.MemRef(address, 4)
    def store(at, offset, number):
        return mir.Op(at, ir.Operation.MOVE, "mov", (), (), kind=mir.Kind.STORE,
                      args=(mir.Const(number, 2),), stores=(mir.MemRef(address.plus(offset), 2),), covers=(at, at+2))
    value = mir.Value(10, 10, variable=10, version=1)
    load = mir.Op(10, ir.Operation.MOVE, "mov", (value,), (), kind=mir.Kind.LOAD,
                  args=(mir.Cell(whole),), results=(mir.Held(value, 4),), loads=(whole,), covers=(10, 12))
    stores = (store(0, 0, 0x5678), store(2, 2, 0x1234)) if complete else (store(0, 0, 0x5678),)
    body = mir.MirBody(0, (mir.MirBlock(0, (), (*stores, load), ()),))
    result = promote.promoted(body, frozenset({5}))
    ops = result.blocks[0].ops
    assert all(op in ops for op in stores)
    assert bool(next(op for op in ops if op.at == 10).loads) is not complete
    if complete:
        assert any(op.kind is mir.Kind.COPY and op.args == (mir.Const(0x12345678, 4),) for op in ops)
