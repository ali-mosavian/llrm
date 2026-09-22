"""`canonical.identities`: the neutral terms a rewrite states in full, folded once."""

from dataclasses import replace

from qbopt.model import ir
from qbopt.model import mir
from qbopt.model import execute
from qbopt.optimize import canonical


def _body(kind: mir.Kind, constant: int, test: mir.Kind) -> mir.MirBody:
    """`t = x kind constant; if t test 0 return 1 else return 2`, x live in."""
    x, t, flags = mir.Value(1, 0), mir.Value(2, 0), mir.Value(3, 0, flags=True)
    ops = (
        mir.computed(0, kind, t, (mir.Held(x, 2), mir.Const(constant, 2)), 2),
        mir.Op(0, ir.Operation.COMPARE, "", (flags,), (t,), kind=mir.Kind.SUB, args=(mir.Held(t, 2), mir.Const(0, 2))),
        mir.Op(0, ir.Operation.BRANCH, "", (), (flags,), kind=mir.Kind.BRANCH, test=test, target=1),
    )
    returned = [
        mir.Op(at, ir.Operation.RETURN, "", (), (), kind=mir.Kind.RETURN, args=(mir.Const(at, 2),)) for at in (1, 2)
    ]
    return mir.MirBody(
        0,
        (
            mir.MirBlock(0, (), ops, (1, 2)),
            mir.MirBlock(1, (), (returned[0],), ()),
            mir.MirBlock(2, (), (returned[1],), ()),
        ),
        sealed=True,
    )


def test_a_neutral_term_is_its_operand_and_a_test_below_zero_is_equality() -> None:
    """Rotation's `bound - 0 + 0` and `bound <=u 0` used to be folded inside the rewrite that wrote them."""
    for kind, constant in ((mir.Kind.ADD, 0), (mir.Kind.SUB, 0), (mir.Kind.MUL, 1)):
        for test in (mir.Kind.BELOW_EQ, mir.Kind.ABOVE):
            body = _body(kind, constant, test)
            folded = canonical.identities(body)
            (entry, *_) = folded.blocks
            assert [op.kind for op in entry.ops] == [mir.Kind.SUB, mir.Kind.BRANCH]
            assert entry.ops[1].test is {mir.Kind.BELOW_EQ: mir.Kind.EQ, mir.Kind.ABOVE: mir.Kind.NE}[test]
            for x in (0, 1, 0xFFFF):
                inputs = {mir.Value(1, 0): x}
                assert execute.run(folded, inputs).returned == execute.run(body, inputs).returned


def test_a_term_that_is_not_neutral_stays() -> None:
    body = _body(mir.Kind.SUB, 1, mir.Kind.BELOW)
    assert canonical.identities(body) is body


def test_a_folded_term_keeps_the_source_bytes_it_owned() -> None:
    """NESTED's raised `add x,0` vanished outright and its bytes were refused as not instructions."""
    body = _body(mir.Kind.ADD, 0, mir.Kind.BELOW)
    (entry, *rest) = body.blocks
    owned = replace(entry.ops[0], id=7, source_backed=True)
    body = replace(body, blocks=(replace(entry, ops=(owned, *entry.ops[1:])), *rest))

    (entry, *_) = canonical.identities(body).blocks
    assert [(op.kind, op.id) for op in entry.ops][0] == (mir.Kind.NOTHING, 7)
