"""IVARM's selected field must receive 34, not its counter's exit value 37."""

from pathlib import Path
from dataclasses import replace
from unittest.mock import patch

import pytest

from qbopt.model import mir
from qbopt.analysis import loops
from qbopt.frontend import blocks
from qbopt.objectfile import module
from qbopt.optimize import transform
from qbopt.optimize import loopmotion
from qbopt.model.passes import Options


@pytest.mark.parametrize("tag", ["q-O", "p-g2", "v-g3"])
@pytest.mark.parametrize("bound,expected", [(37, 34), (7, None), (38, None)])
def test_last_counter_store_requires_an_exact_nonempty_trip_count(tag, bound, expected):
    """IVARM wrote 7,10,...,34 repeatedly; sinking must store 34 and preserve zero trips."""
    found = module.load(Path(f"fixtures/regressions/ivarm-{tag}.obj"))
    body = mir.bodies(found, blocks.partition(found, blocks.code_map(found)))[0][1]
    body = transform.applied(body, found.dgroup, found.calls, found=found, options=Options(unroll=False))
    (loop,) = loops.loops(body.blocks, body.entry)
    selected = next(
        block for block in body.blocks if block.at in loop.body and block.at != loop.header and len(block.succ) == 2
    )
    branch = selected.ops[-1]
    selected = replace(
        selected,
        succ=(branch.target,),
        ops=(
            *selected.ops[:-1],
            replace(branch, kind=mir.Kind.JUMP, uses=(), args=(), test=None, name="", raised=None),
        ),
    )
    body = replace(body, blocks=tuple(selected if block.at == selected.at else block for block in body.blocks))
    with patch.object(loopmotion, "sunk_stores", lambda body, *args: body):
        body = transform.applied(body, found.dgroup, found.calls, options=Options(unroll=False))
    (loop,) = loops.loops(body.blocks, body.entry)
    header = body.block(loop.header)
    compare, branch = header.ops
    header = replace(header, ops=(replace(compare, args=(compare.args[0], mir.Const(bound, 2))), branch))
    body = replace(body, blocks=tuple(header if block.at == header.at else block for block in body.blocks))
    (store,) = [op for block in body.blocks if block.at in loop.body for op in block.ops if op.stores]
    result = loopmotion.sunk_stores(body, found.dgroup, module.landmarks(found))
    owners = [(block.at, op) for block in result.blocks for op in block.ops if op.stores == store.stores]
    assert len(owners) == 1
    at, after = owners[0]
    if expected is None:
        assert at in loop.body and after.args == store.args
    else:
        assert at not in loop.body
        assert after.args == (mir.Const(expected, 2),)
