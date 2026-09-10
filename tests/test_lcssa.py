"""Loop-closed SSA keeps every loop result behind an exit phi."""

from qbopt.model import ir, mir
from qbopt.optimize import lcssa


def operation(
    at: int,
    kind: mir.Kind,
    defines: tuple[mir.Value, ...] = (),
    uses: tuple[mir.Value, ...] = (),
    args: tuple[mir.Arg, ...] = (),
    results: tuple[mir.Arg, ...] = (),
) -> mir.Op:
    return mir.Op(
        at,
        ir.Operation.MOVE,
        "mov",
        defines,
        uses,
        kind=kind,
        args=args,
        results=results,
        covers=(at, at),
    )


def loop_with_exit_use() -> tuple[mir.MirBody, mir.Value, mir.Op]:
    seed = mir.Value(1, 0, variable=1, version=1)
    carried = mir.Value(2, 1, variable=1, version=2)
    stepped = mir.Value(3, 2, variable=1, version=3)
    answer = mir.Value(4, 3, variable=2, version=1)
    initialize = operation(
        0,
        mir.Kind.COPY,
        (seed,),
        args=(mir.Const(0, 2),),
        results=(mir.Held(seed, 2),),
    )
    advance = operation(
        2,
        mir.Kind.ADD,
        (stepped,),
        (carried,),
        (mir.Held(carried, 2), mir.Const(1, 2)),
        (mir.Held(stepped, 2),),
    )
    consume = operation(
        3,
        mir.Kind.COPY,
        (answer,),
        (carried,),
        (mir.Held(carried, 2),),
        (mir.Held(answer, 2),),
    )
    body = mir.MirBody(
        0,
        (
            mir.MirBlock(0, (), (initialize,), (1,)),
            mir.MirBlock(1, (mir.Phi(carried, {0: seed, 2: stepped}),), (), (2, 3)),
            mir.MirBlock(2, (), (advance,), (1,)),
            mir.MirBlock(3, (), (consume,), ()),
        ),
    )
    return body, carried, consume


def test_a_loop_value_used_after_the_exit_gets_an_exit_phi() -> None:
    body, carried, consume = loop_with_exit_use()

    result = lcssa.closed(body)

    exit_block = result.block(3)
    assert exit_block is not None
    assert len(exit_block.phis) == 1
    phi = exit_block.phis[0]
    assert phi.incoming == {1: carried}
    assert phi.result.variable == carried.variable
    assert phi.result.version > carried.version
    changed = exit_block.ops[0]
    assert changed != consume
    assert changed.uses == (phi.result,)
    assert changed.args == (mir.Held(phi.result, 2),)


def test_loop_closed_ssa_is_idempotent() -> None:
    body, _, _ = loop_with_exit_use()
    once = lcssa.closed(body)
    assert lcssa.closed(once) == once


def test_a_value_already_consumed_by_an_exit_phi_is_closed() -> None:
    body, carried, _ = loop_with_exit_use()
    result = mir.Value(8, 3, variable=8, version=1)
    exit_block = body.block(3)
    assert exit_block is not None
    body = mir.MirBody(
        body.entry,
        tuple(
            mir.MirBlock(block.at, (mir.Phi(result, {1: carried}),), (), block.succ)
            if block.at == exit_block.at
            else block
            for block in body.blocks
        ),
    )
    assert lcssa.closed(body) == body


def test_multiple_exit_edges_are_left_for_loop_simplify() -> None:
    body, _, _ = loop_with_exit_use()
    latch = body.block(2)
    assert latch is not None
    body = mir.MirBody(
        body.entry,
        tuple(mir.MirBlock(block.at, block.phis, block.ops, (1, 3)) if block.at == latch.at else block
              for block in body.blocks),
    )
    assert lcssa.closed(body) == body
