from pathlib import Path
from dataclasses import replace

from test_lcssa import loop_with_exit_use

import corpus
from qbopt.model import mir
from qbopt.analysis import loops
from qbopt.optimize import lcssa
from qbopt.optimize import transform
from qbopt.model.passes import Options


def multiple_exits() -> tuple[mir.MirBody, mir.Value]:
    body, carried, consume = loop_with_exit_use()
    body = replace(
        body,
        blocks=(
            body.blocks[0],
            body.blocks[1],
            replace(body.blocks[2], succ=(1, 4)),
            mir.MirBlock(3, (), (), (5,)),
            mir.MirBlock(4, (), (), (5,)),
            mir.MirBlock(5, (), (replace(consume, at=5),), ()),
        ),
    )
    return body, carried


def test_distinct_exits_merge_before_the_downstream_use() -> None:
    body, carried = multiple_exits()
    result = lcssa.closed(body)
    assert result is not body
    first, second, join = (result.block(at) for at in (3, 4, 5))
    assert first is not None and second is not None and join is not None
    assert first.phis[0].incoming == {1: carried}
    assert second.phis[0].incoming == {2: carried}
    assert join.phis[0].incoming == {3: first.phis[0].result, 4: second.phis[0].result}
    assert join.ops[0].uses == (join.phis[0].result,)
    assert lcssa.closed(result) is result


def test_bypass_phi_keeps_its_non_loop_input() -> None:
    body, carried = multiple_exits()
    seed = body.blocks[0].ops[0].defines[0]
    answer = mir.Value(20, 6, variable=2)
    body = replace(
        body,
        blocks=(
            replace(body.blocks[0], succ=(1, 6)),
            *body.blocks[1:-1],
            mir.MirBlock(5, (), (), (6,)),
            mir.MirBlock(6, (mir.Phi(answer, {0: seed, 5: carried}),), (), ()),
        ),
    )
    result = lcssa.closed(body)
    join = result.block(5)
    bypass = result.block(6)
    assert join is not None and bypass is not None
    assert join.phis
    assert bypass.phis[0].incoming == {0: seed, 5: join.phis[0].result}
    assert lcssa.closed(result) is result


def test_direct_use_after_a_bypass_is_not_fabricated() -> None:
    body, _ = multiple_exits()
    body = replace(body, blocks=(replace(body.blocks[0], succ=(1, 5)), *body.blocks[1:]))
    assert lcssa.closed(body) is body


def test_following_cycle_keeps_complete_phi_edges() -> None:
    body, _ = multiple_exits()
    join = body.blocks[-1]
    body = replace(body, blocks=(*body.blocks[:-1], replace(join, succ=(5, 6)), mir.MirBlock(6, (), (), ())))
    result = lcssa.closed(body)
    assert result is not body
    predecessors = loops.predecessors(result.blocks)
    for block in result.blocks:
        for phi in block.phis:
            assert set(phi.incoming) == predecessors[block.at]
    assert lcssa.closed(result) is result


def test_compiled_early_exit_accumulator_is_closed() -> None:
    path = Path("fixtures/regressions/lcmerge-p-g2.obj")
    found = corpus.loaded(path)
    assert found is not None
    partition = corpus.partitioned(path)
    body = transform.applied(
        mir.bodies(found, partition)[0][1],
        found.dgroup,
        found.calls,
        blocks=partition,
        found=found,
        options=Options(lcssa=False),
    )
    (loop,) = loops.loops(body.blocks, body.entry)
    result = lcssa.closed(body)
    assert result != body
    defined = {
        value
        for block in result.blocks
        if block.at in loop.body
        for value in (*(phi.result for phi in block.phis), *(value for op in block.ops for value in op.defines))
        if not value.flags
    }
    outside = [block for block in result.blocks if block.at not in loop.body]
    assert not any(value in defined for block in outside for op in block.ops for value in op.uses)
    assert all(
        parent in loop.body
        for block in outside
        for phi in block.phis
        for parent, value in phi.incoming.items()
        if value in defined
    )
    assert lcssa.closed(result) is result
