"""Evaluate storage-rounded floating recurrences, without reassociation."""

from pathlib import Path
from dataclasses import replace

import pytest
import corpus

from qbopt.analysis import consts, floatfacts
from qbopt.model import mir


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
    path = Path(f"fixtures/omf/fpcse-{tag}.obj")
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
            divisor = next(op.args[1].ref for op in ops if op.kind is mir.Kind.FDIV)
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
