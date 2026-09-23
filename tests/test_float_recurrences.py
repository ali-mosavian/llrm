"""Evaluate storage-rounded floating recurrences, without reassociation."""

from pathlib import Path
from dataclasses import replace

import pytest
import corpus

from qbopt.analysis import consts, floatfacts
from qbopt.model import mir


def test_proved_exit_folds_a_read_without_removing_strict_operations():
    """FPCSE's 487.5 exit was reported but unavailable to later integer reads."""
    from qbopt.optimize import transform
    path = Path("fixtures/omf/fpcse-p-g2.obj")
    found = corpus.loaded(path)
    body = mir.bodies(found, corpus.partitioned(path))[0][1]
    latch = next(block for block in body.blocks if any(op.floating for op in block.ops))
    accumulator = next(op.stores[0] for op in reversed(latch.ops) if op.kind is mir.Kind.FSTORE)
    exit_block = next(block for block in body.blocks if any(op.kind is mir.Kind.CALL for op in block.ops))
    value = mir.Value(10000, exit_block.at, variable=10000)
    read = replace(exit_block.ops[0], kind=mir.Kind.LOAD, args=(mir.Cell(accumulator),),
                   results=(mir.Held(value, 4),), defines=(value,), uses=(),
                   loads=(accumulator,), stores=(), merges={}, source_backed=False, raised=None)
    body = replace(body, blocks=tuple(replace(block, ops=(read, *block.ops))
                                     if block is exit_block else block for block in body.blocks))
    changed = transform.folded(body, found.dgroup, found.calls)
    result = next(op for block in changed.blocks for op in block.ops if value in op.defines)
    assert result.kind is mir.Kind.COPY and result.args == (mir.Const(0x43f3c000, 4),)
    assert next(block for block in changed.blocks if block.at == latch.at) == latch


@pytest.mark.parametrize("change", ["none", "call", "alias", "bypass"])
def test_exit_facts_stay_on_the_proved_edge_and_obey_memory_effects(change):
    """487.5 is the normal FPCSE exit, not an invariant or a post-call guarantee."""
    path = Path("fixtures/omf/fpcse-p-g2.obj")
    found = corpus.loaded(path)
    body = mir.bodies(found, corpus.partitioned(path))[0][1]
    latch = next(block for block in body.blocks if any(op.floating for op in block.ops))
    accumulator = next(op.stores[0] for op in reversed(latch.ops) if op.kind is mir.Kind.FSTORE)
    edges = floatfacts.exit_cells(body, found.dgroup, found.calls)
    (header, destination), = edges
    exit_block = next(block for block in body.blocks if block.at == destination)
    index = 0
    match change:
        case "call":
            index = next(index + 1 for index, op in enumerate(exit_block.ops) if op.kind is mir.Kind.CALL)
        case "alias":
            store = replace(exit_block.ops[0], kind=mir.Kind.STORE, stores=(mir.MemRef(None, 4),))
            body = replace(body, blocks=tuple(replace(block, ops=(store, *block.ops))
                                             if block is exit_block else block for block in body.blocks))
            index = 1
        case "bypass":
            body = replace(body, blocks=tuple(replace(block, succ=(*block.succ, destination))
                                             if block.at == body.entry else block for block in body.blocks))
    memory = consts.cells(body, found.dgroup, found.calls, consts.known(body, found.dgroup, found.calls), edges=edges)
    assert consts._cell(memory[header, 0], accumulator) is None
    assert consts._cell(memory[latch.at, 0], accumulator) is None
    assert consts._cell(memory[destination, index], accumulator) == (
        consts.Known(0x43f3c000, 4) if change == "none" else None)


@pytest.mark.parametrize("count,bits", [(3, 0x43124000), (10, 0x43f3c000)])
def test_loop_exit_analysis_uses_the_actual_bound(count, bits):
    """FPCSE's exit must follow its loop bound, not an assumed ten iterations."""
    path = Path("fixtures/omf/fpcse-p-g2.obj")
    found = corpus.loaded(path)
    body = mir.bodies(found, corpus.partitioned(path))[0][1]
    body = replace(body, blocks=tuple(replace(block, ops=tuple(
        replace(op, args=(op.args[0], mir.Const(count, op.args[1].width)))
        if op.kind is mir.Kind.SUB and len(op.args) == 2 and op.args[1] == mir.Const(10, 2) else op
        for op in block.ops)) for block in body.blocks))
    proofs = floatfacts.loop_exits(body, found.dgroup, found.calls)
    assert len(proofs) == 1 and proofs[0].count == count
    latch = next(block for block in body.blocks if any(op.floating for op in block.ops))
    accumulator = next(op.stores[0] for op in reversed(latch.ops) if op.kind is mir.Kind.FSTORE)
    assert dict(proofs[0].stores)[accumulator] == consts.Known(bits, 4)


@pytest.mark.parametrize("change", ["bound", "header_alias", "header_call"])
def test_loop_exit_analysis_rejects_unproved_control_or_header_effects(change):
    path = Path("fixtures/omf/fpcse-p-g2.obj")
    found = corpus.loaded(path)
    body = mir.bodies(found, corpus.partitioned(path))[0][1]
    header = next(block for block in body.blocks if block.phis and block.ops[-1].kind is mir.Kind.BRANCH)
    def changed(op):
        if change == "bound" and op.kind is mir.Kind.SUB:
            return replace(op, args=(op.args[0], mir.Opaque(None, "unknown bound")))
        if op.stores:
            if change == "header_alias":
                return replace(op, stores=(mir.MemRef(None, 2),))
            if change == "header_call":
                return replace(op, kind=mir.Kind.CALL)
        return op
    body = replace(body, blocks=tuple(replace(block, ops=tuple(map(changed, block.ops)))
                                     if block is header else block for block in body.blocks))
    assert floatfacts.loop_exits(body, found.dgroup, found.calls) == ()


def test_stage_dump_exposes_proved_loop_exit(capsys):
    from tools import stages
    path = Path("fixtures/omf/fpcse-p-g2.obj")
    found = corpus.loaded(path)
    stages._mir(mir.bodies(found, corpus.partitioned(path)), found)
    report = capsys.readouterr().out
    assert "after 10 iterations" in report and "0x43f3c000" in report


@pytest.mark.parametrize("tag", ["p-g2", "v-g3"])
def test_fpcse_memory_recurrence_has_exact_single_exit(tag):
    """FPCSE retained ten iterations although its rounded accumulator exits at 487.5."""
    path = Path(f"fixtures/omf/fpcse-{tag}.obj".lower())
    found = corpus.loaded(path)
    body = mir.bodies(found, corpus.partitioned(path))[0][1]
    entry = next(block for block in body.blocks if block.at == body.entry)
    latch = next(block for block in body.blocks if any(op.floating for op in block.ops))
    known = consts.known(body, found.dgroup, found.calls)
    initial = consts.cells(body, found.dgroup, found.calls, known)[entry.at, len(entry.ops) - 1]
    before = dict(initial)
    result = floatfacts.repeated(latch.ops, 10, initial, found.dgroup, known)
    assert result is not None
    accumulator = next(op.stores[0] for op in reversed(latch.ops) if op.kind is mir.Kind.FSTORE)
    assert consts._cell(result, accumulator) == consts.Known(0x43f3c000, 4)
    assert initial == before


@pytest.mark.parametrize("change", ["inexact", "unknown", "call", "zero", "negative", "budget"])
def test_recurrence_requires_known_exact_steps_and_bounded_work(change):
    """A 6/7 quotient must not become an invented exact loop-exit constant."""
    path = Path("fixtures/omf/fpcse-p-g2.obj")
    found = corpus.loaded(path)
    body = mir.bodies(found, corpus.partitioned(path))[0][1]
    entry = body.blocks[0]
    latch = next(block for block in body.blocks if any(op.floating for op in block.ops))
    known = consts.known(body, found.dgroup, found.calls)
    initial = consts.cells(body, found.dgroup, found.calls, known)[entry.at, len(entry.ops) - 1]
    ops, count = latch.ops, 10
    match change:
        case "inexact":
            divisor_arg = next(op.args[1] for op in ops if op.kind is mir.Kind.FDIV)
            if isinstance(divisor_arg, mir.Cell):
                divisor = divisor_arg.ref
            else:
                assert isinstance(divisor_arg, mir.Held)
                producer = next(
                    op for block in body.blocks for op in block.ops if divisor_arg.value in op.defines
                )
                divisor = next(arg.ref for arg in producer.args if isinstance(arg, mir.Cell))
            initial = {**initial, **consts._fragments(divisor, consts.Known(0x40e00000, 4))}
        case "unknown":
            initial = {}
        case "call":
            ops = (replace(ops[0], kind=mir.Kind.CALL, floating=None), *ops[1:])
        case "zero":
            count = 0
        case "negative":
            count = -1
        case "budget":
            count = 100_001
    result = floatfacts.repeated(ops, count, initial, found.dgroup, known)
    if change == "zero":
        assert result == initial and result is not initial
    else:
        assert result is None
