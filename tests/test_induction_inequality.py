"""Canonical not-equal loops retain a finite, non-wrapping trip-count proof."""

from dataclasses import replace
from pathlib import Path

import pytest

from qbopt.analysis import consts, induction, loops
from qbopt.frontend import blocks
from qbopt.model import mir
from qbopt.objectfile import module
from qbopt.optimize import transform


@pytest.fixture(scope="module")
def counted_loop():
    found = module.load(Path("fixtures/regressions/ivarm-p-g2.obj"))
    body = mir.bodies(found, blocks.partition(found, blocks.code_map(found)))[0][1]
    body = transform.applied(body, found.dgroup, found.calls, found=found, unroll_=False)
    loop, = loops.loops(body.blocks, body.entry)
    counter, = induction.basics(body, loop).values()
    return body, loop, counter


@pytest.mark.parametrize("start,step,bound,last", [
    (7, 3, 37, 34), (37, -3, 7, 10), (0, 1, 32767, 32766),
    (0, -1, -32768, -32767),
    (7, 3, 38, None), (7, -3, 37, None), (7, 0, 37, None),
    (7, 3, 7, None), (32767, 1, -32768, None),
])
@pytest.mark.parametrize("exit_branch", [False, True])
def test_not_equal_loop_reaches_bound_without_wrapping(counted_loop, start, step, bound, last, exit_branch):
    """IVARM lost its ten-iteration proof when IndVarSimplify changed <=10 to !=37."""
    body, loop, counter = counted_loop
    header = body.block(loop.header)
    compare, branch = header.ops
    compare = replace(compare, args=(compare.args[0], mir.Const(bound, 2)))
    branch = replace(branch, test=mir.Kind.EQ if exit_branch else mir.Kind.NE,
                     target=next(at for at in header.succ if (at not in loop.body) == exit_branch))
    body = replace(body, blocks=tuple(replace(block, ops=(compare, branch))
                                     if block.at == header.at else block for block in body.blocks))
    counter = replace(counter, start=mir.Const(start, 2), step=mir.Const(step, 2))
    assert induction._last_counter(body, loop, counter, consts.known(body), 2) == last
