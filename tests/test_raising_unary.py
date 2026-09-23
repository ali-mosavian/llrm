"""Long unary expressions must enter optimization as whole values."""

from pathlib import Path

import corpus
import pytest

from qbopt.model import mir
from qbopt import wholeseg


@pytest.mark.parametrize("tag", ["p-g2", "q-O", "v-g3"])
def test_negnot_raises_printed_long_negations(tag):
    """NEGNOT passed split NEG/ADC/NEG chains to PRINT, blocking whole-value folding."""
    path = Path(f"fixtures/omf/negnot-{tag}.obj".lower())
    found = corpus.loaded(path)
    body = mir.bodies(found, corpus.partitioned(path))[0][1]
    ops = [op for block in body.blocks for op in block.ops]
    assert not any(op.kind is mir.Kind.ADD_CARRY for op in ops)
    assert sum(op.kind is mir.Kind.NEG and op.results[0].width == 4 for op in ops) == 3


@pytest.mark.parametrize("tag", ["p-g2", "q-O", "v-g3"])
def test_nots_stores_whole_unary_results_without_stack_splitting(tag):
    """NOTS split whole EQV/NAND results with PUSH/POP merely to store their halves."""
    result = wholeseg.emitted(Path(f"fixtures/omf/nots-{tag}.obj".lower()).read_bytes())
    assert result.outcome is wholeseg.Emission.LIR, result.reason
    instructions = [str(one.insn) for block in corpus.partitioned(result.data) for one in block.insns]
    assert not any(instruction.startswith("pop ") for instruction in instructions)


@pytest.mark.parametrize("tag", ["p-g2", "q-O", "v-g3"])
def test_arith_passes_whole_results_without_splitting_them(tag):
    """ARITH split eight long results through PUSH/POP just to push the same bytes again."""
    result = wholeseg.emitted(Path(f"fixtures/omf/arith-{tag}.obj".lower()).read_bytes())
    assert result.outcome is wholeseg.Emission.LIR, result.reason
    instructions = [str(one.insn) for block in corpus.partitioned(result.data) for one in block.insns]
    assert not any(instruction.startswith("pop ") for instruction in instructions)


@pytest.mark.parametrize("hazard", ["reversed", "same-half", "separated"])
def test_argument_join_requires_ordered_adjacent_halves(monkeypatch, hazard):
    """Joining low/high, duplicate halves, or separated pushes changes the argument bytes."""
    from dataclasses import replace
    from qbopt.frontend import raising_longs
    path = Path("fixtures/omf/arith-p-g2.obj")
    found = corpus.loaded(path)
    with monkeypatch.context() as context:
        context.setattr(raising_longs, "arguments", lambda body: body)
        bodies = mir.bodies(found, corpus.partitioned(path))
        public = bodies[0][1]
        body = mir._with_raise_context(public, bodies.hints[public.entry], bodies.source)
    block = body.blocks[0]
    index = next(index for index, op in enumerate(block.ops[:-1])
                 if op.kind is mir.Kind.ARG and block.ops[index + 1].kind is mir.Kind.ARG)
    high, low = block.ops[index:index + 2]
    match hazard:
        case "reversed":
            high, low = replace(high, args=low.args, uses=low.uses), replace(low, args=high.args, uses=high.uses)
        case "same-half":
            low = replace(low, args=high.args, uses=high.uses)
        case "separated":
            low = replace(low, covers=(low.covers[0] + 1, low.covers[1] + 1))
    body = replace(body, blocks=(replace(block, ops=(*block.ops[:index], high, low, *block.ops[index + 2:])),))
    raised = raising_longs.arguments(body)
    kept = [op for block in raised.blocks for op in block.ops if op.at in (high.at, low.at)]
    assert all(op in kept for op in (high, low))


@pytest.mark.parametrize("observed", ["low-flags", "high-flags", "carry-value"])
def test_long_negation_keeps_observed_intermediate_results(monkeypatch, observed):
    """Widening a pair must not erase flags or an independently observed intermediate."""
    from dataclasses import replace
    from qbopt.frontend import raising_longs
    path = Path("fixtures/omf/negnot-p-g2.obj")
    found = corpus.loaded(path)
    with monkeypatch.context() as context:
        context.setattr(raising_longs, "unary", lambda body: body)
        bodies = mir.bodies(found, corpus.partitioned(path))
        public = bodies[0][1]
        body = mir._with_raise_context(public, bodies.hints[public.entry], bodies.source)
    block = body.blocks[0]
    index = next(index for index, op in enumerate(block.ops) if op.kind is mir.Kind.NEG)
    low, carry, high = block.ops[index:index + 3]
    match observed:
        case "low-flags":
            value = next(value for value in low.defines if value.flags)
        case "high-flags":
            value = next(value for value in high.defines if value.flags)
        case "carry-value":
            value = carry.results[0].value
    last = replace(block.ops[-1], uses=(*block.ops[-1].uses, value))
    body = replace(body, blocks=(replace(block, ops=(*block.ops[:-1], last)),))
    raised = raising_longs.unary(body)
    assert any(op.at == carry.at and op.kind is mir.Kind.ADD_CARRY
               for block in raised.blocks for op in block.ops)
