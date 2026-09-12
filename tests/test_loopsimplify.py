from pathlib import Path
from dataclasses import replace

import pytest

import corpus
from qbopt.model import mir
from qbopt.analysis import loops
from qbopt.optimize import transform
from qbopt.optimize import loopsimplify


def test_adjacent_angle_loops_reach_a_fixed_point() -> None:
    # PL_MOVE aborted after 16 rounds: Decide erased the preheader each round.
    path = Path("fixtures/regressions/qrender-pl-move-v-g3.obj")
    found = corpus.loaded(path)
    assert found is not None
    body = next(body for name, body in mir.bodies(found, corpus.partitioned(path)) if name.endswith(" MDL_ANGLEMOD"))
    result = transform.applied(body, frozenset(), {}, found=found)
    assert len(loops.loops(result.blocks, result.entry)) == 2


@pytest.mark.parametrize("program", ["nbody", "fpbench"])
def test_real_timer_loop_has_one_backedge(program: str) -> None:
    path = Path(f"fixtures/bench/{program}-v-g3.obj")
    found = corpus.loaded(path)
    assert found is not None
    body = next(body for name, body in mir.bodies(found, corpus.partitioned(path)) if name.endswith("PITSNAP"))
    (original,) = loops.loops(body.blocks, body.entry)
    assert len(original.latches) == 2
    result = loopsimplify.simplified(body)
    (loop,) = loops.loops(result.blocks, result.entry)
    assert len(loop.latches) == 1
    latch = result.block(next(iter(loop.latches)))
    assert latch is not None
    assert loops.predecessors(result.blocks)[latch.at] == original.latches
    predecessors = loops.predecessors(result.blocks)
    for block in result.blocks:
        for phi in block.phis:
            assert set(phi.incoming) == predecessors[block.at]
    assert loopsimplify.simplified(result) is result


def test_grouping_preserves_each_phi_edge_value() -> None:
    from test_lcssa import loop_with_exit_use

    body, carried, _ = loop_with_exit_use()
    entry = body.blocks[0]
    seed = entry.ops[0].defines[0]
    other_seed = mir.Value(20, 4, variable=seed.variable, version=20)
    # Two distinct entries must retain their own source value at the preheader.
    other = replace(
        entry, at=4, ops=(replace(entry.ops[0], at=4, defines=(other_seed,), results=(mir.Held(other_seed, 2),)),)
    )
    header = body.blocks[1]
    header = replace(header, phis=(mir.Phi(carried, {0: seed, 4: other_seed, 2: header.phis[0].incoming[2]}),))
    body = replace(body, blocks=(entry, header, *body.blocks[2:], other))
    result = loopsimplify.grouped(body, 1, frozenset({0, 4}))
    bridge = result.blocks[-1]
    assert bridge.phis[0].incoming == {0: seed, 4: other_seed}
    header = result.block(1)
    latch = result.block(2)
    assert header is not None and latch is not None
    assert header.phis[0].incoming[bridge.at] == bridge.phis[0].result
    assert latch.succ == (1,)


@pytest.mark.parametrize("hazard", ["entry", "missing-source", "opaque", "bad-phi"])
def test_unsupported_group_is_atomic(hazard: str) -> None:
    from test_lcssa import loop_with_exit_use

    body, _, _ = loop_with_exit_use()
    target, sources = 1, frozenset({0})
    if hazard == "entry":
        target = body.entry
    elif hazard == "missing-source":
        sources = frozenset({999})
    elif hazard == "opaque":
        entry = body.blocks[0]
        body = replace(
            body, blocks=(replace(entry, ops=(replace(entry.ops[0], kind=mir.Kind.OPAQUE),)), *body.blocks[1:])
        )
    else:
        header = body.blocks[1]
        phi = header.phis[0]
        body = replace(
            body, blocks=(body.blocks[0], replace(header, phis=(replace(phi, incoming={}),)), *body.blocks[2:])
        )
    assert loopsimplify.grouped(body, target, sources) is body


@pytest.mark.parametrize("unsupported_exit", [False, True])
def test_conditional_entry_and_shared_exit_become_dedicated(unsupported_exit: bool) -> None:
    from test_lcssa import operation
    from test_lcssa import loop_with_exit_use

    body, _, _ = loop_with_exit_use()
    entry, header, latch, exit_block = body.blocks
    entry = replace(
        entry,
        succ=(header.at, exit_block.at),
        ops=(*entry.ops, replace(operation(10, mir.Kind.BRANCH), target=exit_block.at)),
    )
    header = replace(header, ops=(replace(operation(11, mir.Kind.BRANCH), target=exit_block.at),))
    if unsupported_exit:
        header = replace(header, ops=(replace(header.ops[0], kind=mir.Kind.OPAQUE),))
    body = replace(body, blocks=(entry, header, latch, exit_block))
    result = loopsimplify.simplified(body)
    if unsupported_exit:
        assert result is body
        return
    (loop,) = loops.loops(result.blocks, result.entry)
    predecessors = loops.predecessors(result.blocks)
    outside = predecessors[loop.header] - loop.body
    assert len(outside) == 1
    preheader = result.block(next(iter(outside)))
    assert preheader is not None and preheader.succ == (header.at,)
    exits = {at for block in result.blocks if block.at in loop.body for at in block.succ if at not in loop.body}
    assert exits != {exit_block.at}
    assert all(predecessors[at] <= loop.body for at in exits)
    assert loopsimplify.simplified(result) is result
