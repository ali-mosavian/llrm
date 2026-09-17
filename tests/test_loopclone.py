"""Peeling must preserve diamond joins, early exits and the residual loop."""

from dataclasses import replace

import pytest

from qbopt.analysis import loops
from qbopt.model import ir, mir
from qbopt.optimize import loopclone


def diamond():
    values = [mir.Value(index, index, variable=index, version=1) for index in range(1, 8)]
    seed, carried, left, right, selected, stepped, answer = values

    def copy(at, result, source):
        return mir.Op(at, ir.Operation.MOVE, "", (result,), (source,),
                      kind=mir.Kind.COPY, args=(mir.Held(source, 2),),
                      results=(mir.Held(result, 2),))

    def branch(at, target):
        return mir.Op(at, ir.Operation.BRANCH, "", (), (carried,), kind=mir.Kind.BRANCH,
                      target=target, args=(mir.Held(carried, 2),))

    body = mir.MirBody(0, (
        mir.MirBlock(0, (), (), (1,)),
        mir.MirBlock(1, (mir.Phi(carried, {0: seed, 5: stepped}),), (branch(10, 2),), (2, 6)),
        mir.MirBlock(2, (), (branch(20, 3),), (3, 4)),
        mir.MirBlock(3, (), (copy(30, left, carried), branch(31, 6)), (5, 6)),
        mir.MirBlock(4, (), (copy(40, right, carried),), (5,)),
        mir.MirBlock(5, (mir.Phi(selected, {3: left, 4: right}),),
                     (copy(50, stepped, selected),), (1,)),
        mir.MirBlock(6, (mir.Phi(answer, {1: carried, 3: left}),), (), ()),
    ))
    return body, values


def test_peeling_clones_diamond_and_early_exit_phis():
    body, values = diamond()
    loop, = loops.loops(body.blocks, body.entry)
    changed = loopclone.peeled(body, loop, 2)
    assert changed is not None
    assert len(changed.blocks) == len(body.blocks) + 2 * len(loop.body)
    predecessors = loops.predecessors(changed.blocks)
    for block in changed.blocks:
        for phi in block.phis:
            assert set(phi.incoming) == set(predecessors[block.at])
    first = changed.block(changed.block(0).succ[0])
    assert first.phis[0].incoming == {0: values[0]}
    residual = changed.block(1)
    assert residual.phis[0].incoming[5] == values[5]
    assert 0 not in residual.phis[0].incoming
    assert len(changed.block(6).phis[0].incoming) == 6
    defined = [value.id for block in changed.blocks
               for value in (*[phi.result for phi in block.phis],
                             *[value for op in block.ops for value in op.defines])]
    assert len(defined) == len(set(defined))
    assert loops.loops(changed.blocks, changed.entry) == [loop]
    owners = {value: block.at for block in changed.blocks for value in
              (*[phi.result for phi in block.phis],
               *[value for op in block.ops for value in op.defines])}
    dominators = loops.dominators(changed.blocks, changed.entry)
    for block in changed.blocks:
        for op in block.ops:
            assert all(owners[value] in dominators[block.at] for value in op.uses if value in owners)
        for phi in block.phis:
            assert all(owners[value] in dominators[source]
                       for source, value in phi.incoming.items() if value in owners)


def test_clones_read_their_own_values_and_do_not_duplicate_byte_ownership():
    body, values = diamond()
    loop, = loops.loops(body.blocks, body.entry)
    changed = loopclone.peeled(body, loop, 1)
    assert changed is not None
    originals = {block.at for block in body.blocks}
    fresh = {value for block in changed.blocks if block.at not in originals
             for value in (*[phi.result for phi in block.phis],
                           *[value for op in block.ops for value in op.defines])}
    for block in changed.blocks:
        if block.at in originals:
            continue
        for op in block.ops:
            assert set(op.uses) <= fresh
            assert op.inserted and not op.absorbed
            if op.results:
                assert op.results[0].value == op.defines[0]
    assert changed.block(3).ops == body.block(3).ops


def test_peeling_clones_pointer_identity_and_seed_facts() -> None:
    """Matmul's peeled pointer values lost their exact frame-object leaves.

    SSA cloning changes value identity, so semantic pointer classification
    and frontend-established roots must be remapped with the definitions.
    Otherwise alias analysis sees the cloned address arithmetic as ordinary
    integers and SROA cannot scalarize its fixed aggregate accesses.
    """
    from qbopt.model import memory

    body, values = diamond()
    _seed, _carried, left, right, selected, stepped, _answer = values
    object_ = memory.Object(memory.Kind.FRAME, (7, -16, -4), extent=12)
    provenance = memory.Provenance.one(object_, 0, 1)
    pointers = frozenset({left, right, selected, stepped})
    body = replace(body, pointer_values=pointers, pointer_seeds={left: provenance})
    (loop,) = loops.loops(body.blocks, body.entry)

    changed = loopclone.peeled(body, loop, 1)

    assert changed is not None
    originals = {block.at for block in body.blocks}
    copied_results = [
        op.defines[0]
        for block in changed.blocks
        if block.at not in originals
        for op in block.ops
        if op.at in (30, 40, 50) and op.defines
    ]
    assert copied_results
    assert set(copied_results) <= changed.pointer_values
    cloned_left = next(
        op.defines[0]
        for block in changed.blocks
        if block.at not in originals
        for op in block.ops
        if op.at == 30
    )
    assert changed.pointer_seeds[cloned_left] == provenance


def test_unclosed_loop_value_is_refused():
    body, values = diamond()
    exit_block = body.block(6)
    escape = replace(body.block(3).ops[0], uses=(values[1],), args=(mir.Held(values[1], 2),))
    body = replace(body, blocks=(*body.blocks[:-1], replace(exit_block, ops=(escape,))))
    loop, = loops.loops(body.blocks, body.entry)
    assert loopclone.peeled(body, loop, 1) is None


def test_opaque_dispatch_is_not_cloned_as_an_ordinary_branch():
    body, _ = diamond()
    dispatch = body.block(2)
    dispatch = replace(dispatch, ops=(replace(dispatch.ops[0], kind=mir.Kind.CALL),))
    body = replace(body, blocks=tuple(dispatch if block.at == 2 else block for block in body.blocks))
    loop, = loops.loops(body.blocks, body.entry)
    assert loopclone.peeled(body, loop, 1) is None


@pytest.mark.parametrize("count", [0, -1])
def test_nonpositive_peel_count_is_rejected(count):
    body, _ = diamond()
    loop, = loops.loops(body.blocks, body.entry)
    with pytest.raises(ValueError):
        loopclone.peeled(body, loop, count)
