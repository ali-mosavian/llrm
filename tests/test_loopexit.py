"""Exit-value evaluation must remove work, not merely change the loop's syntax."""

from pathlib import Path
from dataclasses import replace

import pytest

from qbopt import blocks, module, omf, wholeseg


@pytest.mark.parametrize("tag", ["p-g2", "q-O", "v-g3"])
@pytest.mark.parametrize("program", ["lngmxx", "hotlpx", "hotlop"])
def test_accumulation_has_no_backedge(tag, program):
    """LNGMXX repeated a fixed sum; HOTLPX/HOTLOP repeated product + index twenty times."""
    result = wholeseg.emitted(Path(f"fixtures/omf/{program}-{tag}.obj").read_bytes())
    assert result.outcome is wholeseg.Emission.LIR, result.reason
    found = module.of(omf.parse(result.data))
    from qbopt import loops
    partition = blocks.partition(found, blocks.code_map(found))
    assert not loops.loops(partition, 0x30)


def _body(monkeypatch, program="lngmxx"):
    import corpus
    from qbopt import loopexit, mir, transform

    path = Path(f"fixtures/omf/{program}-p-g2.obj")
    found = corpus.loaded(path)
    partition = corpus.partitioned(path)
    with monkeypatch.context() as context:
        context.setattr(loopexit, "evaluated", lambda body: body)
        return transform.applied(mir.bodies(found, partition)[0][1], found.dgroup,
                                 found.calls, blocks=partition, found=found)


@pytest.mark.parametrize("hazard", ["zero-trip", "wrapping-control", "store", "live-latch"])
def test_loop_exit_requires_a_complete_proof(monkeypatch, hazard):
    """Deleting LNGMXX's loop must not discard stores, live latch values, or an unproved exit."""
    from qbopt import loopexit, mir

    body = _body(monkeypatch)
    changed = []
    for block in body.blocks:
        ops = list(block.ops)
        if block.at == 0x91 and hazard in ("zero-trip", "wrapping-control"):
            ops[0] = replace(ops[0], args=(ops[0].args[0], mir.Const(0 if hazard == "zero-trip" else 32767, 2)))
        if block.at == 0x50 and hazard == "store":
            store = next(op for other in body.blocks for op in other.ops if op.stores)
            ops.insert(0, store)
        if block.at == 0x99 and hazard == "live-latch":
            value = next(other for other in body.blocks if other.at == 0x50).ops[-1].defines[0]
            ops[0] = replace(ops[0], uses=(value,), args=(mir.Held(value, 2),))
        changed.append(replace(block, ops=tuple(ops)))
    body = replace(body, blocks=tuple(changed))
    assert loopexit.evaluated(body) is body


@pytest.mark.parametrize("start,step", [(0, 14290), (2147483640, 10), (-2147483640, -10)])
def test_accumulation_exit_wraps_at_its_own_width(monkeypatch, start, step):
    """A long sum must keep its modulo-32-bit answer when ten updates overflow."""
    from qbopt import consts, induction, loopexit, loops, mir

    body = _body(monkeypatch)
    loop, = loops.loops(body.blocks, body.entry)
    counter, = [one for one in induction.basics(body, loop).values() if one.start.width == 4]
    values = {counter.start.value: start, counter.step.value: step}
    body = replace(body, blocks=tuple(replace(block, ops=tuple(
        replace(op, kind=mir.Kind.COPY, args=(mir.Const(values[op.defines[0]], 4),), uses=())
        if len(op.defines) == 1 and op.defines[0] in values else op
        for op in block.ops)) for block in body.blocks))
    result = loopexit.evaluated(body)
    assert result is not body
    facts = consts.known(result)
    answer = next(value for value in facts if value.id == counter.value)
    assert facts[answer].n == (start + 10 * step) & 0xffffffff


@pytest.mark.parametrize("start,bound,step", [(1, 20, 1), (1, 300, 1), (20, 1, -1)])
def test_index_sum_uses_the_exact_triangular_coefficient(monkeypatch, start, bound, step):
    """HOTLPX must sum the original index values even when the 16-bit triangular product wraps."""
    from qbopt import consts, induction, loopexit, loops, mir

    body = _body(monkeypatch, "hotlpx")
    loop, = loops.loops(body.blocks, body.entry)
    counter, = induction.basics(body, loop).values()
    header = next(block for block in body.blocks if block.at == loop.header)
    accumulator = next(phi.result for phi in header.phis if phi.result.id != counter.value)
    changed = []
    for block in body.blocks:
        ops = []
        for op in block.ops:
            if op.defines == (counter.start.value,):
                op = replace(op, args=(mir.Const(start, 2),))
            if op.kind is mir.Kind.MUL:
                op = replace(op, kind=mir.Kind.COPY, args=(mir.Const(21, 2),), loads=(), uses=())
            if op.kind is mir.Kind.INCREMENT:
                op = replace(op, kind=mir.Kind.INCREMENT if step == 1 else mir.Kind.DECREMENT)
            if block.at == loop.header and op.kind is mir.Kind.SUB:
                op = replace(op, args=(op.args[0], mir.Const(bound, 2)))
            if block.at == loop.header and op.kind is mir.Kind.BRANCH:
                op = replace(op, test=mir.Kind.LE if step == 1 else mir.Kind.GE)
            ops.append(op)
        changed.append(replace(block, ops=tuple(ops)))
    result = loopexit.evaluated(replace(body, blocks=tuple(changed)))
    assert not loops.loops(result.blocks, result.entry)
    expected = sum(21 + index for index in range(start, bound + step, step)) & 0xffff
    assert consts.known(result)[accumulator].n == expected


def test_a_doubled_accumulator_is_not_a_linear_sum(monkeypatch):
    """Replacing s := 2*s + index by a triangular sum would silently change HOTLPX's answer."""
    from qbopt import loopexit, mir

    body = _body(monkeypatch, "hotlpx")
    body = replace(body, blocks=tuple(replace(block, ops=tuple(
        replace(op, args=(op.args[1], op.args[1]), uses=(op.args[1].value,))
        if op.at == 0x56 and op.kind is mir.Kind.ADD else op
        for op in block.ops)) for block in body.blocks))
    assert loopexit.evaluated(body) is body
