"""
qbopt/analysis/loops.py's own gate: the shapes BC actually compiles, and the one it
never does.
"""

from pathlib import Path

import pytest

import corpus
from qbopt.analysis import loops
from qbopt.frontend.blocks import Ends
from qbopt.frontend.blocks import Block

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


def test_dead_predecessor_does_not_erase_live_dominance() -> None:
    """A dead branch into a live join erased its entry dominator; no runtime miscompile observed."""
    chain = [block(0, (1,)), block(1, (2,)), block(2, ()), block(9, (2,))]
    assert loops.dominators(chain) == {
        0: frozenset({0}), 1: frozenset({0, 1}),
        2: frozenset({0, 1, 2}), 9: frozenset(),
    }


def test_dead_predecessor_has_no_dominance_frontier() -> None:
    """A dead edge into a live join invented a frontier and unnecessary phi sites."""
    chain = [block(0, (1,)), block(1, (2,)), block(2, ()), block(9, (2,))]
    assert loops.frontiers(chain) == dict.fromkeys((0, 1, 2, 9), frozenset())


def test_disconnected_cycle_has_no_dominators_or_natural_loops() -> None:
    """An unreachable cycle retained every block as a dominator and invented natural loops."""
    chain = [block(0, (1,)), block(1, ()), block(8, (9,)), block(9, (8,))]
    assert loops.dominators(chain)[8] == frozenset()
    assert loops.dominators(chain)[9] == frozenset()
    assert loops.loops(chain) == []
    assert loops.irreducible(chain) == frozenset()


def test_dead_edge_into_latch_is_not_part_of_live_loop() -> None:
    """A dead edge hid a live loop; its dead source must not enter the recovered body."""
    chain = [block(0, (1,)), block(1, (2, 3)), block(2, (1,)),
             block(3, ()), block(9, (2,))]
    assert loops.loops(chain) == [loops.Loop(1, frozenset({2}), frozenset({1, 2}))]


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
    # Two, not one: matrix, nested, spill and segld are written with a
    # nested FOR because that is what an optimiser has to see through --
    # an invariant in an inner loop costs the outer loop's trip count
    # times over. Before they existed nothing here nested at all, which
    # is what this asserted.
    assert max(everything_else) == 2, "nothing but RESUME nests past two loops"
    assert min(resumable) > 1, "divmod is the /X program, and RESUME is why"


def test_the_entry_dominates_everything_and_nothing_dominates_it() -> None:
    chain = [block(0, (1,)), block(1, (2,)), block(2, (), Ends.RETURN)]
    assert loops.immediate_dominators(chain)[0] is None
    assert loops.immediate_dominators(chain)[2] == 1


def test_a_frontier_is_where_two_definitions_could_meet() -> None:
    """A diamond: both arms are on the frontier of the join, the head is not.

    The head dominates the join, so a definition there reaches it by every
    path and needs no phi. Either arm dominates only itself, so control
    arrives at the join both through it and around it.
    """
    diamond = [block(0, (1, 2)), block(1, (3,)), block(2, (3,)), block(3, (), Ends.RETURN)]
    found = loops.frontiers(diamond)
    assert found[1] == frozenset({3})
    assert found[2] == frozenset({3})
    assert found[0] == frozenset(), "the head dominates the join"


def test_a_loop_header_is_on_its_own_frontier() -> None:
    chain = [block(0, (1,)), block(1, (2, 3)), block(2, (1,)), block(3, (), Ends.RETURN)]
    assert 1 in loops.frontiers(chain)[2], "the latch reaches the header around the entry path"


def test_a_block_with_one_predecessor_is_never_a_frontier() -> None:
    chain = [block(0, (1,)), block(1, (2,)), block(2, (), Ends.RETURN)]
    assert all(1 not in where and 2 not in where for where in loops.frontiers(chain).values())
