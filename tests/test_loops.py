"""
qbopt/loops.py's own gate: the shapes BC actually compiles, and the one it
never does.
"""

from pathlib import Path

import pytest

import corpus
from qbopt import loops
from qbopt.blocks import Ends
from qbopt.blocks import Block

FIXTURES = sorted(Path("fixtures/omf").glob("*.obj"))


def block(at: int, succ: tuple[int, ...], ends: Ends = Ends.CONDITIONAL) -> Block:
    """A block with no instructions -- only its edges matter here."""
    return Block(at=at, end=at + 1, insns=(), ends=ends, succ=succ)


def test_a_straight_line_has_no_loops() -> None:
    chain = [block(0, (1,)), block(1, (2,)), block(2, (), Ends.RETURN)]
    assert loops.loops(chain) == []
    assert loops.depth(chain) == {0: 0, 1: 0, 2: 0}


def test_a_self_loop_is_its_own_body() -> None:
    """The case that pulls in the whole preceding graph if the walk starts anyway."""
    chain = [block(0, (1,)), block(1, (1, 2)), block(2, (), Ends.RETURN)]
    found = loops.loops(chain)
    assert len(found) == 1
    assert found[0].header == 1
    assert found[0].latches == frozenset({1})
    assert found[0].body == frozenset({1}), "block 0 is not inside the loop"


def test_a_test_at_the_bottom_loop_is_reducible() -> None:
    """BC's own FOR: jump to the test, body above it, branch back up into it.

    The edge into the body retreats through the address space, which is
    exactly why irreducible() cannot decide by address order.
    """
    chain = [
        block(0, (2,)),  # jmp test
        block(1, (2,)),  # body -> test
        block(2, (1, 3)),  # test -> body (backwards) or exit
        block(3, (), Ends.RETURN),
    ]
    assert loops.irreducible(chain) == frozenset()
    found = loops.loops(chain)
    assert len(found) == 1
    assert found[0].header == 2, "the test dominates the body, so the test is the header"
    assert found[0].body == frozenset({1, 2})


def test_a_cycle_entered_at_two_blocks_is_irreducible() -> None:
    """Two ways in, so no single block dominates the cycle -- BASIC's GOTO."""
    chain = [block(0, (1, 2)), block(1, (2,)), block(2, (1,)), block(3, (), Ends.RETURN)]
    assert loops.irreducible(chain) != frozenset()


def test_nesting_counts_every_enclosing_loop() -> None:
    outer_only = [
        block(0, (1,)),
        block(1, (2, 4)),  # outer header
        block(2, (3,)),  # inner header
        block(3, (2, 1)),  # inner latch, then outer latch
        block(4, (), Ends.RETURN),
    ]
    found = loops.depth(outer_only)
    assert found[0] == 0
    assert found[2] > found[1], "the inner block sits inside both loops"


def test_an_unreachable_block_dominates_nothing() -> None:
    chain = [block(0, (1,)), block(1, (), Ends.RETURN), block(9, (1,))]
    assert loops.dominators(chain)[9] == frozenset()


@pytest.mark.parametrize("obj", FIXTURES, ids=lambda p: p.stem)
def test_every_fixture_is_reducible(obj: Path) -> None:
    """Measured across the corpus: BC never emits an irreducible graph.

    So the refusal irreducible() exists for is defensive, not a case anything
    here has had to handle -- worth knowing before an optimizer is designed
    around needing to.
    """
    found = corpus.loaded(obj)
    assert found is not None
    mapped = corpus.mapped(obj)
    assert not isinstance(mapped, str), mapped
    assert loops.irreducible(corpus.partitioned(obj)) == frozenset()


@pytest.mark.parametrize("obj", FIXTURES, ids=lambda p: p.stem)
def test_a_loop_body_always_contains_its_own_header_and_latch(obj: Path) -> None:
    found = corpus.loaded(obj)
    assert found is not None
    mapped = corpus.mapped(obj)
    assert not isinstance(mapped, str), mapped
    partitioned = corpus.partitioned(obj)
    known = {one.at for one in partitioned}
    for loop in loops.loops(partitioned):
        assert loop.header in loop.body
        assert loop.latches <= loop.body
        assert loop.body <= known, "a loop body never names a block outside the graph"


def test_resume_dispatch_is_the_only_deep_nesting_in_the_corpus() -> None:
    """RESUME's own dispatch block, and nothing else, nests more than one deep.

    Every path the handler can return to dominates the dispatch, so each is a
    natural loop and the block reports a depth in the twenties. It is what
    the definition says, not a defect -- but it is the reason depth() cannot
    be trusted on a body built with /X, which is out of scope anyway.
    """
    deepest: dict[str, int] = {}
    for obj in FIXTURES:
        found = corpus.loaded(obj)
        if found is None:
            continue
        mapped = corpus.mapped(obj)
        if isinstance(mapped, str):
            continue
        nesting = loops.depth(corpus.partitioned(obj))
        deepest[obj.stem] = max(nesting.values()) if nesting else 0

    resumable = [d for stem, d in deepest.items() if stem.startswith("divmod")]
    everything_else = [d for stem, d in deepest.items() if not stem.startswith("divmod")]
    assert max(everything_else) == 1, "nothing but RESUME nests past one loop"
    assert min(resumable) > 1, "divmod is the /X program, and RESUME is why"
