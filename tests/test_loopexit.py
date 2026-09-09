"""Exit-value evaluation must remove work, not merely change the loop's syntax."""

from pathlib import Path
from dataclasses import replace

import pytest

from qbopt import blocks, module, omf, wholeseg


@pytest.mark.parametrize("tag", ["p-g2", "q-O", "v-g3"])
def test_lngmxx_has_no_accumulation_backedge(tag):
    """LNGMXX added its invariant quotient/remainder sum ten times instead of multiplying once."""
    result = wholeseg.emitted(Path(f"fixtures/omf/lngmxx-{tag}.obj").read_bytes())
    assert result.outcome is wholeseg.Emission.LIR, result.reason
    found = module.of(omf.parse(result.data))
    from qbopt import loops
    partition = blocks.partition(found, blocks.code_map(found))
    assert not loops.loops(partition, 0x30)


def _body(monkeypatch):
    import corpus
    from qbopt import loopexit, mir, transform

    path = Path("fixtures/omf/lngmxx-p-g2.obj")
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
