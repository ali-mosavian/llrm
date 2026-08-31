"""
qbopt/regalloc.py's own gate: liveness is where an allocator goes wrong
quietly, so the invariants that bound it are the test.
"""

from pathlib import Path

import pytest

import corpus
from qbopt import ir
from qbopt import mir
from qbopt import regalloc

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


@pytest.mark.parametrize("obj", FIXTURES, ids=lambda p: p.stem)
def test_no_body_needs_more_registers_than_bc_used(obj: Path) -> None:
    """The program came out of six registers, so at no point can seven
    values have been live. A number above that is a liveness bug and not a
    body that needs spilling -- which is what it was twice, first from
    attributing phi arguments to the join instead of the edge, then from
    never killing a value the caller supplied.
    """
    for body in raised(obj):
        assert regalloc.pressure(body) <= len(regalloc.AVAILABLE)


@pytest.mark.parametrize("obj", FIXTURES, ids=lambda p: p.stem)
def test_a_value_never_interferes_with_another_version_of_itself(obj: Path) -> None:
    """Two versions of eax live at once would mean BC had them in one
    register at one moment, which it cannot have. Measured over 27,680
    values: none. This is what makes the identity assignment always valid,
    and so what the whole allocator rests on.

    Asks body.origin rather than the value: a value no longer carries where
    it lived, which is the point of docs/variables.md -- and this is one of
    the few questions genuinely about BC's registers, so it asks for them.
    """
    for body in raised(obj):
        origin = body.origin
        for one, others in regalloc.interference(body).items():
            assert not [other for other in others if origin.get(other) is origin.get(one)]


@pytest.mark.parametrize("obj", FIXTURES, ids=lambda p: p.stem)
def test_an_unconstrained_body_moves_nothing(obj: Path) -> None:
    """Every value back where BC had it. An allocator that moves what it was
    not asked to move emits copies for nothing -- greedy by degree did,
    1,274 of them, all valid and all pointless."""
    for body in raised(obj):
        got = regalloc.colour(body)
        assert not isinstance(got, str), got
        assert regalloc.moved(body, got) == 0


@pytest.mark.parametrize("obj", FIXTURES, ids=lambda p: p.stem)
def test_a_colouring_gives_interfering_values_different_registers(obj: Path) -> None:
    for body in raised(obj):
        got = regalloc.colour(body)
        assert not isinstance(got, str), got
        for one, others in regalloc.interference(body).items():
            for other in others:
                if other in got and one in got:
                    assert got[one] is not got[other], f"{one} and {other} share {got[one]}"


def test_an_entry_value_is_defined_where_the_body_starts() -> None:
    """A body reads a register before writing it whenever BC passes
    something in. Nothing defines it, so a backward liveness that does not
    treat the entry as its definition keeps it live at every point that can
    reach its use -- which around a loop is everywhere."""
    seen = 0
    for body in raised(Path("fixtures/omf/divmod-p-g2-zd.obj")):
        arriving = regalloc.entry_values(body)
        if not arriving:
            continue
        seen += 1
        found = regalloc.live(body)
        assert not (arriving & found.live_in[body.entry]), "defined on entry, so not live into it"
    assert seen, "divmod's own main body reads registers BC passed in"


@pytest.mark.parametrize("obj", FIXTURES[:20], ids=lambda p: p.stem)
def test_a_pin_that_cannot_be_honoured_is_refused_not_guessed(obj: Path) -> None:
    """Pre-colouring breaks the chordal guarantee, so colouring can fail --
    and failing has to be a refusal. Pinning every value in a body to one
    register is the extreme case of that."""
    for body in raised(obj):
        graph = regalloc.interference(body)
        clashing = [one for one, others in graph.items() if others]
        if len(clashing) < 2:
            continue
        pinned = {one: regalloc.AVAILABLE[0] for one in clashing}
        assert isinstance(regalloc.colour(body, pinned), str)
        return
