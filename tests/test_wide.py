"""
qbopt/wide.py's own gate: the pair is a graph property, and the check that
two halves belong together is the part that fails silently if it is wrong.
"""

from pathlib import Path

import pytest

import corpus
from qbopt import ir
from qbopt import mir
from qbopt import wide
from qbopt.module import Addr
from qbopt.module import Space

FIXTURES = sorted(Path("fixtures/omf").glob("*.obj"))
SOMEWHERE = Addr(Space.SEGMENT, 0x10, 5)


def half(name: str, addr: Addr | None) -> mir.Op:
    loads = (mir.MemRef(addr, 2),) if addr is not None else ()
    return mir.Op(0, ir.Operation.BINARY, name, (), (), loads, ())


def bodies(obj: Path) -> list[mir.MirBody]:
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
        built = mir.raise_body(mine, nodes, body.body.seed)
        assert not isinstance(built, str), built
        out.append(built)
    return out


@pytest.mark.parametrize(
    ("low", "high", "agree", "why"),
    [
        (SOMEWHERE, SOMEWHERE.plus(2), True, "the high half is the two bytes above the low"),
        (SOMEWHERE, SOMEWHERE.plus(4), False, "four apart is not one long"),
        (SOMEWHERE, SOMEWHERE, False, "the same address twice is not two halves"),
        (SOMEWHERE, None, False, "one half in memory and one not is not a pair"),
        (None, None, True, "BC's own negate: the high half takes an immediate"),
    ],
)
def test_two_halves_belong_together_or_they_do_not(low: Addr | None, high: Addr | None, agree: bool, why: str) -> None:
    """Getting this wrong computes a different number rather than failing.

    lift.py's `+2` is the same rule, and this exists because a check that
    never refuses is not a check -- the corpus happens to refuse none, so
    the refusals have to be shown to be reachable here.
    """
    assert wide._halves_agree(half("add", low), half("adc", high)) is agree, why


def test_the_pair_is_found_through_the_carry_not_the_layout() -> None:
    """The point of raising: `add` defines a flags value and `adc` reads
    that exact value, so the pair is a question about the graph rather than
    about where BC put the two instructions."""
    seen = 0
    for body in bodies(Path("fixtures/omf/arith-v-g3.obj")):
        for pair in wide.pairs(body):
            assert pair.carry in pair.low.defines
            assert pair.carry in pair.high.uses
            seen += 1
    assert seen, "arith is a program of 32-bit arithmetic"


@pytest.mark.parametrize("obj", FIXTURES, ids=lambda p: p.stem)
def test_every_folded_pair_is_a_shape_folds_names(obj: Path) -> None:
    for body in bodies(obj):
        for pair in wide.pairs(body):
            assert (pair.low.name, pair.high.name) in wide.FOLDS
            assert pair.op == wide.FOLDS[(pair.low.name, pair.high.name)]


@pytest.mark.parametrize("obj", FIXTURES, ids=lambda p: p.stem)
def test_a_carry_has_exactly_one_producer(obj: Path) -> None:
    """SSA's own promise, relied on here: the flags value an adc reads was
    defined once, so "which add produced it" has one answer."""
    for body in bodies(obj):
        for pair in wide.pairs(body):
            makers = [op for block in body.blocks for op in block.ops if pair.carry in op.defines]
            assert makers == [pair.low]


def test_folding_retires_the_carry_it_consumed() -> None:
    for body in bodies(Path("fixtures/omf/arith-v-g3.obj")):
        found = wide.pairs(body)
        before, after = wide.freed(body, found)
        assert after == before - len(found)
