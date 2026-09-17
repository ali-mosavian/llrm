"""MIR call expansion preserves SSA and refuses unmodelled call results."""

from pathlib import Path
from collections import Counter

from qbopt.model import ir
from qbopt.model import mir
from qbopt.optimize import inline
from qbopt.cfront import compile as cfront

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


def test_inline_policy_requires_one_surviving_call() -> None:
    """Inlining a multiply-called body duplicates it; the initial policy has
    no pressure model strong enough to justify that expansion yet.
    """
    leaf = _leaf()
    assert (
        inline.candidates(
            {"leaf": leaf},
            {"leaf": ()},
            Counter({"leaf": 2}),
            frozenset({"leaf"}),
            frozenset({"leaf"}),
            call_cost=4,
        )
        == {}
    )


def test_small_private_pure_helpers_inline_in_mir() -> None:
    """GCC and Clang inline choose's one-use pick/which helpers. Keeping both
    calls paid six argument pushes, two calls and two cleanups, while hiding
    the shared condition from the caller optimizer.
    """
    lines = [
        line.strip()
        for line in cfront.compiled((FIXTURES / "choose.cgs").read_text(), "choose", optimise=True).splitlines()
    ]
    assert "_pick proc near" not in lines
    assert "_which proc near" not in lines
    body = lines[lines.index("_choose proc far") : lines.index("_choose endp")]
    assert "call _pick" not in body
    assert "call _which" not in body
