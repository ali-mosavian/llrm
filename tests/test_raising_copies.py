"""Copy semantics retain BC's pointer results, ordering, and environment gates."""

from dataclasses import replace
from pathlib import Path

import corpus
import pytest

from qbopt.analysis import consts
from qbopt.backend import lower
from qbopt.frontend import declen, raising_copies, raising_literals
from qbopt.model import ir, mir


def _instruction(raw):
    decoded = declen.decode(bytes(0x14b) + raw, 0x14b)
    node = ir.Opaque(decoded, ir.instruction_effects(decoded, lambda *_: None))
    return mir.Op(decoded.at, ir.Operation.BARRIER, "", (), (), node=node,
                  covers=(decoded.at, decoded.end),
                  loads=tuple(mir.MemRef(cell.addr, cell.width) for cell in node.effects.loads),
                  stores=tuple(mir.MemRef(cell.addr, cell.width) for cell in node.effects.stores))


def _copy(byte=0xfc):
    path = Path("fixtures/omf/fpdeep-p-g2.obj")
    found = corpus.loaded(path)
    body = mir.bodies(found, corpus.partitioned(path))[0][1]
    ops = tuple(op for block in body.blocks for op in block.ops if 0x14c <= op.at <= 0x157)
    if byte is not None:
        ops = (_instruction(bytes([byte])), *ops)
    return found, replace(body, initial=(), blocks=(mir.MirBlock(body.entry, (), ops, ()),))


@pytest.mark.parametrize("byte,step", [(0xfc, 2), (0xfd, -2)])
def test_copy_has_explicit_memory_and_pointer_results(byte, step):
    """FPDEEP's d=12 needs memory effects, not eight unused pointer definitions."""
    found, body = _copy(byte)
    raised = raising_copies.scalar(body, found)
    reads = [op for op in raised.blocks[0].ops if op.kind is mir.Kind.LOAD]
    writes = [op for op in raised.blocks[0].ops if op.kind is mir.Kind.STORE]
    assert [op.loads[0].addr.disp for op in reads] == [0x22 + step * index for index in range(4)]
    assert [op.stores[0].addr.disp for op in writes] == [0x1a + step * index for index in range(4)]
    assert all(store.args == load.results for load, store in zip(reads, writes))
    pointers = [op for op in raised.blocks[0].ops if op.merges and op.at >= 0x154]
    assert not pointers
    lowered = lower.lowered("copy", raised, {}, {}, {})
    assert sum(one.what is not None and one.what.op is ir.Operation.MOVE for one in lowered.insns) == 8


def test_only_observed_pointer_results_cross_the_raise_boundary():
    """Reading the last copy's pointer must not resurrect its three intermediate updates."""
    found, body = _copy()
    original = body.blocks[0].ops
    last = original[-1].defines[0]
    cell = mir.MemRef(None, 4)
    observe = mir.Op(0x158, ir.Operation.MOVE, "mov", (), (last,), kind=mir.Kind.STORE,
                     args=(mir.Held(last, 4),), stores=(cell,), results=(mir.Cell(cell),), covers=(0x158, 0x158))
    body = replace(body, blocks=(replace(body.blocks[0], ops=(*original, observe)),))
    result = raising_copies.scalar(body, found)
    updates = [op for op in result.blocks[0].ops if op.kind is mir.Kind.COPY and op.at >= 0x154]
    assert len(updates) == 1 and updates[0].defines == (last,)
    first = next(op for op in original if op.at == 0x154)
    before = next(value for value in first.uses if body.origin[value] == body.origin[last])
    assert updates[0].merges == {before: last}
    assert updates[0].results == (mir.Held(last, 2),)


def test_pointer_used_on_a_successor_edge_survives():
    """Copy outputs consumed by a phi are uses even without a local reader."""
    found, body = _copy()
    entry = body.blocks[0]
    last = entry.ops[-1].defines[0]
    joined = replace(last, id=last.id + 1000, at=0x200, version=last.version + 1)
    successor = mir.MirBlock(0x200, (mir.Phi(joined, {entry.at: last}),), (), ())
    body = replace(body, blocks=(replace(entry, succ=(successor.at,)), successor))
    result = raising_copies.scalar(body, found)
    updates = [op for op in result.blocks[0].ops if op.kind is mir.Kind.COPY and op.at >= 0x154]
    assert len(updates) == 1 and updates[0].defines == (last,)


def test_forward_copy_propagates_the_double_literal():
    """The FPDEEP initializer is binary64 12, not an unknown write or an entry value of d."""
    found, body = _copy()
    body = raising_literals.initialized(raising_copies.scalar(body, found), found)
    known = consts.known(body, found.dgroup, {})
    stores = [op for op in body.blocks[0].ops if op.kind is mir.Kind.STORE]
    assert [known[op.args[0].value].n for op in stores] == [0, 0, 0, 0x4028]


@pytest.mark.parametrize("conflict", [b"", b"\xfd", b"\x1f", bytes.fromhex("9a00000000")])
def test_copy_environment_survives_only_agreeing_predecessors(conflict):
    """FPDEEP's d=12 copy must not become unknown just because setup crosses an edge.

    A backwards incoming path must still prevent folding it to binary64 12.
    """
    found, body = _copy()
    entry = body.blocks[0]
    setup, copying = entry.ops[:5], entry.ops[5:]
    left = mir.MirBlock(0x180, (), (), (0x200,))
    right = mir.MirBlock(0x190, (), (_instruction(conflict),) if conflict else (), (0x200,))
    join = mir.MirBlock(0x200, (), copying, ())
    body = replace(body, blocks=(replace(entry, ops=setup, succ=(left.at, right.at)), left, right, join))
    raised = raising_copies.scalar(body, found)
    stores = [op for op in raised.blocks[-1].ops if op.kind is mir.Kind.STORE]
    assert len(stores) == (0 if conflict else 4)
    reordered = replace(body, blocks=(join, right, left, body.blocks[0]))
    raised = raising_copies.scalar(reordered, found)
    assert sum(op.kind is mir.Kind.STORE for op in raised.blocks[0].ops) == len(stores)


def test_copy_selects_without_clobbering_arithmetic_flags():
    """MOVSW preserves arithmetic flags even though its pointer offsets change."""
    from qbopt import flow
    from qbopt.backend import frame, select

    found, body = _copy()
    low = lower.lowered("copy", raising_copies.scalar(body, found), {}, {}, {})
    for stage in flow.machine({}, frame.of(low, {}), {}):
        low = stage.transform(low)
    for op in low.insns:
        if op.what and op.what.op is ir.Operation.MOVE:
            encoded = select.emit(op.what)
            assert encoded is not None
            assert declen.decode(encoded.code, 0).writes == 0


def test_copy_does_not_advance_a_symbol_beyond_its_segment():
    """A near-pointer wrap is not an address in the next relocated segment."""
    from qbopt.objectfile import omf

    found, body = _copy()
    limits = omf.segments(found.records)
    ops = tuple(replace(op, args=(replace(op.args[0], offset=limits[op.args[0].index][1] - 2),))
                if op.at == 0x14f else op for op in body.blocks[0].ops)
    body = replace(body, blocks=(replace(body.blocks[0], ops=ops),))
    assert raising_copies.scalar(body, found) == body


def test_proven_copy_unlocks_strict_floating_cse():
    """FPDEEP's repeated DOUBLE load can be shared once d=12 is represented."""
    from qbopt.optimize import transform

    found, body = _copy()
    path = Path("fixtures/omf/fpdeep-p-g2.obj")
    original = mir.bodies(found, corpus.partitioned(path))[0][1]
    floating = tuple(op for block in original.blocks for op in block.ops if 0x158 <= op.at <= 0x174)
    floating = tuple(replace(op, floating_origin=replace(op.floating_origin, block=body.entry))
                     if op.floating_origin else op for op in floating)
    block = body.blocks[0]
    body = replace(body, blocks=(replace(block, ops=(*block.ops, *floating)),))
    body = raising_literals.initialized(raising_copies.scalar(body, found), found)
    result = transform.subexpressions(body, found.dgroup)
    assert sum(op.kind is mir.Kind.FLOAD for block in result.blocks for op in block.ops) == 1
    low = lower.lowered("copy", result, {}, {}, {})
    from qbopt.backend import floatalloc
    low = floatalloc.allocated(low)
    assert any(one.what and one.what.name == "fld" and one.what.sources == (ir.St(0),)
               for one in low.insns)


@pytest.mark.parametrize("change", ["unknown_direction", "unknown_selector", "changed_data_segment", "call"])
def test_unproved_copy_environment_is_not_assumed(change):
    found, body = _copy(None if change == "unknown_direction" else 0xfc)
    ops = list(body.blocks[0].ops)
    if change == "unknown_selector":
        ops = [op for op in ops if op.at != 0x152]
    elif change in ("changed_data_segment", "call"):
        ops.insert(1, _instruction(bytes.fromhex("1f" if change == "changed_data_segment" else "9a00000000")))
    body = replace(body, blocks=(replace(body.blocks[0], ops=tuple(ops)),))
    result = raising_copies.scalar(body, found)
    assert result == body
