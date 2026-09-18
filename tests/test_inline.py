"""MIR call expansion preserves SSA and refuses unmodelled call results."""

from pathlib import Path
from collections import Counter

import pytest

from qbopt.model import ir
from qbopt.model import mir
from qbopt.model import memory
from qbopt.analysis import alias
from qbopt.optimize import inline
from qbopt.cfront import compile as cfront
from qbopt.backend import cpu

FIXTURES = Path(__file__).resolve().parents[1] / "fixtures" / "c"


def _copy(at: int, value: mir.Value, number: int) -> mir.Op:
    return mir.Op(
        at,
        ir.Operation.NOTHING,
        "",
        (value,),
        (),
        kind=mir.Kind.COPY,
        args=(mir.Const(number, 2),),
        results=(mir.Held(value, 2),),
    )


def _leaf() -> mir.MirBody:
    value = mir.Value(1, 1, variable=1, version=1)
    returned = mir.Op(
        2,
        ir.Operation.NOTHING,
        "",
        (),
        (value,),
        kind=mir.Kind.RETURN,
        args=(mir.Held(value, 2),),
    )
    return mir.MirBody(1, (mir.MirBlock(1, (), (_copy(1, value, 37), returned), ()),), sealed=True)


def _caller(*, use_clobber: bool = False) -> tuple[mir.MirBody, mir.Value]:
    result = mir.Value(10, 2, variable=10, version=1)
    clobber = mir.Value(11, 2, variable=11, version=1)
    call = mir.Op(
        2,
        ir.Operation.NOTHING,
        "",
        (result, clobber),
        (),
        kind=mir.Kind.CALL,
        results=(mir.Held(result, 2), mir.Held(clobber, 2)),
    )
    ops = [call]
    if use_clobber:
        observed = mir.Value(12, 3, variable=12, version=1)
        ops.append(
            mir.Op(
                3,
                ir.Operation.NOTHING,
                "",
                (observed,),
                (clobber,),
                kind=mir.Kind.COPY,
                args=(mir.Held(clobber, 2),),
                results=(mir.Held(observed, 2),),
            )
        )
    joined = mir.Value(13, 4, variable=10, version=2)
    returned = mir.Op(
        4,
        ir.Operation.NOTHING,
        "",
        (),
        (joined,),
        kind=mir.Kind.RETURN,
        args=(mir.Held(joined, 2),),
    )
    return (
        mir.MirBody(
            1,
            (
                mir.MirBlock(1, (), tuple(ops), (4,)),
                mir.MirBlock(4, (mir.Phi(joined, {1: result}),), (returned,), ()),
            ),
            sealed=True,
        ),
        result,
    )


def test_inline_splices_return_before_the_original_successor_phi() -> None:
    """The call block used to be a phi predecessor.  Expansion inserts a
    continuation, which must take over that edge without losing the result.
    """
    body, result = _caller()
    made = inline.expanded(body, {2: "leaf"}, {2: frozenset()}, {"leaf": inline.Candidate(_leaf(), ())})

    assert made is not body
    assert not [op for block in made.blocks for op in block.ops if op.kind is mir.Kind.CALL]
    successor = made.block(4)
    assert successor is not None
    assert set(successor.phis[0].incoming) != {1}
    assert set(successor.phis[0].incoming.values()) == {result}
    assert mir.verify(made) == []


def test_inline_refuses_a_live_unmodelled_call_result() -> None:
    """A short C result is semantic AX plus a DX clobber.  DX is normally
    dead, but a caller that observes it cannot have the call silently erased.
    """
    body, _ = _caller(use_clobber=True)
    assert inline.expanded(body, {2: "leaf"}, {2: frozenset()}, {"leaf": inline.Candidate(_leaf(), ())}) is body


def test_inline_policy_refuses_repeated_work_without_a_call_cost() -> None:
    """A repeated body must not clone when the profile cannot price a call."""
    leaf = _leaf()
    assert (
        inline.candidates(
            {"leaf": leaf},
            {"leaf": ()},
            Counter({"leaf": 2}),
            frozenset({"leaf"}),
            frozenset({"leaf"}),
            call_cost=0,
        )
        == {}
    )


def test_small_private_pure_helpers_inline_in_mir() -> None:
    """GCC and Clang inline choose's one-use pick/which helpers. Keeping both
    calls paid six argument pushes, two calls and two cleanups, while hiding
    the shared condition from the caller optimizer.
    """
    stages = []
    lines = [
        line.strip()
        for line in cfront.compiled(
            (FIXTURES / "choose.cgs").read_text(),
            "choose",
            optimise=True,
            watch=lambda stage, name, body: stages.append(body)
            if name == "_choose" and stage == "mir-inline1"
            else None,
        ).splitlines()
    ]
    assert "_pick proc near" not in lines
    assert "_which proc near" not in lines
    body = lines[lines.index("_choose proc far") : lines.index("_choose endp")]
    assert "call _pick" not in body
    assert "call _which" not in body

    facts = alias.points_to(stages[-1])
    addresses = [
        result
        for block in stages[-1].blocks
        for op in block.ops
        if op.kind is mir.Kind.COPY and len(op.args) == 1 and isinstance(op.args[0], mir.Symbol)
        for result in op.defines
        if result in stages[-1].pointer_values
    ]
    assert addresses
    assert all({one.object.kind for one in facts.values[value].slices} == {memory.Kind.GLOBAL} for value in addresses)
    assert not any("offset L_" in line for line in body), (
        "both inlined choices are object addresses, so their null test is true and neither address is needed"
    )


@pytest.mark.parametrize("target", cpu.names())
def test_tiny_private_leaf_inlines_at_two_call_sites(target: str) -> None:
    """inline_twice kept two near calls to one add-only helper.

    The callee has one semantic operation, while every call has argument
    setup, a near call, and cleanup. Keeping it out of line solely because
    the body has two callers leaves that paid work in the caller on every
    target profile.
    """
    source = FIXTURES / "inline_twice.c"
    assembly = cfront.compiled(cfront.recorded(source, []), source.stem, optimise=True, cpu=target)
    lines = [line.strip() for line in assembly.splitlines()]

    assert "_increment proc near" not in lines
    body = lines[lines.index("_inlineTwice proc far") : lines.index("_inlineTwice endp")]
    assert "call _increment" not in body
