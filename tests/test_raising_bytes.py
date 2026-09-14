"""NBODY's PIT snapshot printed zero when high-byte clears lost their SSA result."""
from dataclasses import replace
from pathlib import Path
from unittest.mock import patch

import corpus
from qbopt.frontend import raising_bytes
from qbopt.model import mir


def test_nbody_high_byte_clear_defines_the_word_its_consumer_reads():
    path = Path("fixtures/bench/nbody-v-g3.obj")
    bodies = mir.bodies(corpus.loaded(path), corpus.partitioned(path))
    op = next(op for _, body in bodies for block in body.blocks for op in block.ops if op.at == 0x464)
    assert len(op.results) == 1 and isinstance(op.results[0], mir.Held)
    assert op.results[0].width == 2
    assert op.args[1] == mir.Const(255, 2)
    assert op.merges == {op.args[0].value: op.results[0].value}


def test_high_byte_clear_keeps_observable_flags():
    path = Path("fixtures/bench/nbody-v-g3.obj")
    with patch.object(raising_bytes, "scalar", lambda body: body):
        bodies = mir.bodies(corpus.loaded(path), corpus.partitioned(path))
    body = next(body for _, body in bodies if any(op.at == 0x464 for block in body.blocks for op in block.ops))
    op = next(op for block in body.blocks for op in block.ops if op.at == 0x464)
    block = replace(body.blocks[0], ops=(op,), phis=(), succ=())
    isolated = replace(body, blocks=(block,))
    assert raising_bytes.scalar(isolated).blocks[0].ops[0].kind is mir.Kind.AND, "unread flags do not keep the clear"
    flags = tuple(value for value in op.defines if value.flags)
    reader = replace(op, kind=mir.Kind.NOTHING, uses=flags, defines=(), args=(), results=())
    block = replace(block, ops=(op, reader, op))
    observed = replace(body, blocks=(block,))
    assert raising_bytes.scalar(observed).blocks[0].ops[0] == op
