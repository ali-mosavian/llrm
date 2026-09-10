"""Cyclic SSA joins must distinguish pending values from runtime inputs."""

import pytest
from dataclasses import replace

from qbopt.analysis import consts
from qbopt.model import ir, mir


def body_with_cycle(step=0, external=False):
    start, joined, carried, incoming = (mir.Value(index, 0) for index in range(1, 5))
    seed = mir.Op(0, ir.Operation.MOVE, "mov", (start,), (), kind=mir.Kind.COPY,
                  args=(mir.Const(7, 4),), results=(mir.Held(start, 4),))
    source = incoming if external else joined
    update = mir.Op(10, ir.Operation.BINARY, "add", (carried,), (source,), kind=mir.Kind.ADD,
                    args=(mir.Held(source, 4), mir.Const(step, 4)), results=(mir.Held(carried, 4),))
    body = mir.MirBody(0, (
        mir.MirBlock(0, (), (seed,), (10,)),
        mir.MirBlock(10, (mir.Phi(joined, {0: start, 10: carried}),), (update,), (10, 20)),
        mir.MirBlock(20, (), (), ()),
    ))
    return body, joined, carried


def test_unchanged_loop_value_is_constant_through_its_backedge():
    body, joined, carried = body_with_cycle()
    facts = consts.known(body)
    assert facts[joined] == facts[carried] == consts.Known(7, 4)


@pytest.mark.parametrize("step,external", [(1, False), (0, True)])
def test_changed_or_runtime_backedge_is_not_the_initial_constant(step, external):
    body, joined, carried = body_with_cycle(step, external)
    facts = consts.known(body)
    assert joined not in facts and carried not in facts


def test_unanchored_cycle_does_not_invent_a_constant():
    body, joined, carried = body_with_cycle()
    header = body.blocks[1]
    header = replace(header, phis=(mir.Phi(joined, {10: carried}),))
    body = replace(body, blocks=(body.blocks[0], header, body.blocks[2]))
    assert joined not in consts.known(body)


def test_cyclic_propagation_does_not_widen_a_known_word():
    body, joined, carried = body_with_cycle()
    seed = body.blocks[0].ops[0]
    seed = replace(seed, args=(mir.Const(7, 2),), results=(mir.Held(seed.defines[0], 2),))
    body = replace(body, blocks=(replace(body.blocks[0], ops=(seed,)), *body.blocks[1:]))
    facts = consts.known(body)
    assert joined not in facts and carried not in facts


def test_block_order_does_not_change_cyclic_facts():
    body, _, _ = body_with_cycle()
    reordered = replace(body, blocks=tuple(reversed(body.blocks)))
    assert consts.known(body) == consts.known(reordered)
