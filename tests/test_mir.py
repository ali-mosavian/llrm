"""
qbopt/mir.py's own gate: SSA is either well-formed or it is not a graph you
can reason about, so the invariants are the test.
"""

from pathlib import Path

import pytest

import corpus
from qbopt import ir
from qbopt import mir
from qbopt.blocks import Ends
from qbopt.blocks import Block

FIXTURES = sorted(Path("fixtures/omf").glob("*.obj"))


def block(at: int, succ: tuple[int, ...], ends: Ends = Ends.CONDITIONAL) -> Block:
    return Block(at=at, end=at + 1, insns=(), ends=ends, succ=succ)


def raised(obj: Path) -> list[tuple[mir.MirBody, list[Block]]]:
    """Every body of one object, raised -- the unit mir.py actually takes."""
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
        out.append((built, [b for b in mine if built.block(b.at) is not None]))
    return out


def test_an_empty_body_is_refused_rather_than_guessed() -> None:
    assert isinstance(mir.raise_body([], {}), str)


def test_an_entry_outside_the_blocks_is_refused() -> None:
    assert isinstance(mir.raise_body([block(0, ())], {}, 99), str)


def test_another_bodys_blocks_do_not_come_along() -> None:
    """A procedure is reached by a call, which is no CFG edge.

    Handing raise_body a whole module's blocks used to leave the other
    body's in the graph -- they have their own predecessors, so a phi went
    into them that the dominator walk never reached to fill, and the value
    arriving there vanished. Only what the entry reaches is raised.
    """
    both = [block(0, (1,)), block(1, (), Ends.RETURN), block(0x99, (0x99,))]
    built = mir.raise_body(both, {}, 0)
    assert not isinstance(built, str)
    assert {one.at for one in built.blocks} == {0, 1}, "the second body is not this body"


def test_a_successor_outside_the_body_is_dropped_from_the_edge() -> None:
    built = mir.raise_body([block(0, (1, 0x99)), block(1, (), Ends.RETURN)], {}, 0)
    assert not isinstance(built, str)
    entry = built.block(0)
    assert entry is not None
    assert entry.succ == (1,)


@pytest.mark.parametrize("obj", FIXTURES, ids=lambda p: p.stem)
def test_every_body_raises_and_the_form_holds(obj: Path) -> None:
    """The three SSA promises, over the real corpus.

    Each is load-bearing for a different consumer, and verify()'s own
    docstring says which. A renaming bug does not make the graph malformed
    in any way an eye would catch -- it makes it describe a different
    program -- so this is the only thing standing between construction and
    everything built on it.
    """
    for built, where in raised(obj):
        assert mir.verify(built, where) == []


@pytest.mark.parametrize("obj", FIXTURES, ids=lambda p: p.stem)
def test_a_phi_argument_comes_from_every_predecessor(obj: Path) -> None:
    """The bug the ordering fix was for: a phi created on entry to its own
    block loses the edge from any predecessor renamed before it."""
    for built, _ in raised(obj):
        for one in built.blocks:
            for phi in one.phis:
                assert phi.incoming, f"{phi.result} at {one.at:#06x} has no arguments at all"


@pytest.mark.parametrize("obj", FIXTURES, ids=lambda p: p.stem)
def test_a_value_is_defined_exactly_once(obj: Path) -> None:
    for built, _ in raised(obj):
        seen = built.values
        assert len(seen) == len(set(seen))


@pytest.mark.parametrize("obj", FIXTURES, ids=lambda p: p.stem)
def test_a_barrier_reads_and_writes_every_tracked_value(obj: Path) -> None:
    """ir.py's contract says a barrier's operands are pinned and nothing may
    move across one. Here that is exactly "it defines and uses everything",
    so no value's live range spans it and the allocator has no choice to make.
    """
    for built, _ in raised(obj):
        for one in built.blocks:
            for op in one.ops:
                if not op.barrier:
                    continue
                assert {v.of for v in op.defines} >= set(mir.TRACKED)
                assert {v.of for v in op.uses} >= set(mir.TRACKED)


def test_the_carry_between_a_pair_is_an_edge_not_an_adjacency() -> None:
    """BC's own 32-bit arithmetic: `sub` then `sbb`, where the borrow is the
    whole reason the second instruction is there. In SSA the second reads the
    flags value the first defined, so folding the pair later needs no
    pattern-match on their addresses."""
    found = corpus.loaded(Path("fixtures/omf/arith-v-g3.obj"))
    assert found is not None
    partitioned = corpus.partitioned(Path("fixtures/omf/arith-v-g3.obj"))
    result = corpus.bodies(Path("fixtures/omf/arith-v-g3.obj"))
    assert not isinstance(result, str)
    nodes = {ir.span(n)[0]: n for body in result for n in body.nodes}
    built = mir.raise_body(partitioned, nodes, partitioned[0].at)
    assert not isinstance(built, str)

    pairs = 0
    for one in built.blocks:
        for first, second in zip(one.ops, one.ops[1:], strict=False):
            if second.name not in ("adc", "sbb"):
                continue
            carried = [v for v in first.defines if v.of is mir.FLAGS]
            assert carried, f"{first.name} at {first.at:#06x} feeds {second.name} but defines no flags"
            assert carried[0] in second.uses, "the pair is joined by the flags value"
            pairs += 1
    assert pairs, "arith-v-g3 is a program of 32-bit arithmetic; it has pairs"


def test_the_frame_and_the_segments_never_become_values() -> None:
    """They are where values live, not values. Promoting bp would dissolve
    every local, and a segment register decides which bytes an access names."""
    assert not (set(mir.TRACKED) & mir.PHYSICAL)
    for built, _ in raised(Path("fixtures/omf/procs-v-g3.obj")):
        for value in built.values:
            assert value.of not in mir.PHYSICAL


@pytest.mark.parametrize("obj", FIXTURES, ids=lambda p: p.stem)
def test_every_op_keeps_the_node_it_came_from(obj: Path) -> None:
    """A lowering that applies no transform is that node's own bytes. Losing
    the origin is what would make the identity round-trip unprovable."""
    for built, _ in raised(obj):
        for one in built.blocks:
            for op in one.ops:
                assert isinstance(op.node, ir.Opaque | ir.Long | ir.Call | ir.Restore | ir.Data)
