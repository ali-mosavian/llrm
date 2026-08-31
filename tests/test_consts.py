"""
qbopt/consts.py's own gate: a fold that is wrong produces a plausible number,
so the width rule and what seeds it are the test.
"""

from pathlib import Path

import pytest

import corpus
from qbopt import ir
from qbopt import mir
from qbopt import consts

FIXTURES = sorted(Path("fixtures/omf").glob("*.obj"))


def raised(obj: Path) -> list[mir.MirBody]:
    found = corpus.loaded(obj)
    assert found is not None
    partitioned = corpus.partitioned(obj)
    result = corpus.bodies(obj)
    if isinstance(result, str) or not partitioned:
        return []
    nodes = {ir.span(n)[0]: n for body in result for n in body.nodes}
    out = []
    for body in result:
        mine = [b for b in partitioned if any(lo <= b.at < hi for lo, hi in body.body.ranges)]
        if not mine:
            continue
        built = mir.raise_body(mine, nodes, body.body.seed, found.calls)
        assert not isinstance(built, str), built
        out.append(built)
    return out


@pytest.mark.parametrize(
    ("n", "width", "want"),
    [(5, 2, 5), (-1, 2, 0xFFFF), (0x1FFFF, 2, 0xFFFF), (-1, 4, 0xFFFFFFFF)],
)
def test_a_fact_is_masked_to_its_own_width(n: int, width: int, want: int) -> None:
    assert consts.masked(n, width) == want


def test_flags_do_not_stop_an_operation_being_folded() -> None:
    """Nearly every arithmetic instruction defines its result and the flags
    together, so a rule wanting one definition rejects all of them. It did,
    and the propagation found nothing but its own seeds until this."""
    result = mir.Value(1, mir.Register.EAX, 0)
    flags = mir.Value(2, mir.FLAGS, 0)
    assert consts._defined(mir.Op(0, ir.Operation.UNARY, "dec", (flags, result), ())) == result
    assert consts._defined(mir.Op(0, ir.Operation.UNARY, "dec", (flags,), ())) is None


@pytest.mark.parametrize("obj", FIXTURES, ids=lambda p: p.stem)
def test_a_fact_never_claims_more_bytes_than_the_instruction_wrote(obj: Path) -> None:
    """`mov ax,5` does not make eax five. Claiming it would fold a 32-bit
    use of a value only half of which is known, and the answer would look
    entirely reasonable."""
    for body in raised(obj):
        for fact in consts.known(body).values():
            assert fact.width in (1, 2, 4)
            assert 0 <= fact.n < (1 << (fact.width * 8))


@pytest.mark.parametrize("obj", FIXTURES, ids=lambda p: p.stem)
def test_every_known_value_is_defined_by_an_operation_that_computes_it(obj: Path) -> None:
    for body in raised(obj):
        facts = consts.known(body)
        defined = {consts._defined(op): op for block in body.blocks for op in block.ops}
        for value in facts:
            assert value in defined, f"{value} is known but nothing defines it"
            node = defined[value].node
            assert node is not None
            assert ir.modelled(node.semantics)


def test_a_comparison_result_folds_to_basics_own_true() -> None:
    """BC materialises a comparison as `mov ax,0` then a conditional `dec ax`
    -- and -1 is what BASIC calls true. There is one reaching definition and
    so no phi, which is why the fact survives the block boundary at all."""
    seen = 0
    for body in raised(Path("fixtures/omf/cmpord-p-evt.obj")):
        facts = consts.known(body)
        for block in body.blocks:
            for op in block.ops:
                target = consts._defined(op)
                if op.name == "dec" and target in facts:
                    assert facts[target] == consts.Known(0xFFFF, 2)
                    seen += 1
    assert seen, "cmpord materialises comparison results"


def test_nothing_is_folded_through_a_phi() -> None:
    """A phi is where two definitions meet, so its value is not one of them.
    Folding one needs the conditional half of a sparse propagation, which
    consts.py says it does not do -- this holds it to that."""
    for obj in FIXTURES[:20]:
        for body in raised(obj):
            facts = consts.known(body)
            for block in body.blocks:
                for phi in block.phis:
                    assert phi.result not in facts


def test_division_is_not_folded() -> None:
    """BC's own divide has semantics this pass already refuses to reproduce
    -- calls.py on `x/0` and the signed extreme -- so a folder that answered
    them here would be inventing a result the running program never
    produces."""
    assert "idiv" not in consts.ARITH
    assert "div" not in consts.ARITH
    assert not (set(consts.ARITH) & set(consts.UNARY)), "one table each, no operation in both"


def test_a_fact_is_never_wider_than_the_operation_that_made_it() -> None:
    """`mov ax,5` makes the low half five and says nothing above it. Taking
    the number without the width folds a 32-bit use of a half-known value
    and gives a plausible answer that is not the program's."""
    assert consts.masked(0x1FFFF, 2) == 0xFFFF
    assert consts.Known(5, 2).width == 2
