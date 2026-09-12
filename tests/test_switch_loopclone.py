from dataclasses import replace

from test_loopclone import diamond

from qbopt.model import mir
from qbopt.analysis import loops
from qbopt.optimize import loopclone


def test_loop_cloning_remaps_switch_cases_and_default() -> None:
    body, _ = diamond()
    branch = body.blocks[2].ops[-1]
    switch = replace(branch, kind=mir.Kind.SWITCH, target=3, cases=((1, 3), (2, 4)))
    body = replace(
        body,
        blocks=tuple(
            replace(block, ops=(*block.ops[:-1], switch)) if block.at == 2 else block for block in body.blocks
        ),
    )
    (loop,) = loops.loops(body.blocks, body.entry)
    changed = loopclone.peeled(body, loop, 2)
    assert changed is not None
    copies = [
        block for block in changed.blocks if block.at != 2 and block.ops and block.ops[-1].kind is mir.Kind.SWITCH
    ]
    assert len(copies) == 2
    for block in copies:
        op = block.ops[-1]
        assert tuple(number for number, _ in op.cases) == (1, 2)
        assert {op.target, *(target for _, target in op.cases)} == set(block.succ)
        assert set(block.succ).isdisjoint({3, 4})
    predecessors = loops.predecessors(changed.blocks)
    assert all(set(phi.incoming) == set(predecessors[block.at]) for block in changed.blocks for phi in block.phis)
