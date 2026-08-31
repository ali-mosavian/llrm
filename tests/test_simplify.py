"""
qbopt/simplify.py's own gate.

A deletion that is wrong is silent, so what is tested here is not that the
round trips are found but that nothing else is: the identity has to hold
for the same reason every time, and the register at both ends has to agree.
"""

from pathlib import Path

import pytest

import corpus
from qbopt import mir
from qbopt import simplify
from qbopt.module import Space

FIXTURES = sorted(Path("fixtures/omf").glob("*.obj"))
REWRITTEN = Path("build/bench/v-g3/NBODY.OBJ")


def bodies(obj: Path) -> tuple:
    found = corpus.loaded(obj)
    assert found is not None
    return found, mir.bodies(found, corpus.partitioned(obj))


@pytest.mark.parametrize("obj", FIXTURES, ids=lambda p: p.stem)
def test_bc_alone_has_no_round_trips(obj: Path) -> None:
    """BC never emits the idiom. It exists only after absorption has run.

    So a hit on an unrewritten object would mean this is matching something
    it does not understand, which is exactly the failure a deletion hides.
    """
    _, raised = bodies(obj)
    for _, body in raised:
        assert simplify.round_trips(body) == ()


@pytest.mark.parametrize("obj", FIXTURES, ids=lambda p: p.stem)
def test_every_round_trip_names_one_register_at_both_ends(obj: Path) -> None:
    """What makes the deletion safe rather than merely correct.

    Removing the idiom leaves the register holding the value, so every later
    instruction finds what it expected. A rejoin landing somewhere else
    would need a `mov` emitting, and is refused instead.
    """
    found, raised = bodies(obj)
    for _, body in raised:
        for trip in simplify.round_trips(body):
            assert body.origin.get(trip.value) is trip.register


def test_a_real_round_trip_is_found_and_proved() -> None:
    """bench/nbody.bas, after absorption: the worked example in the module.

    v152 = concat(high16(v150), low16(v150)) = v150, which needs all three
    of docs/variables.md's stages -- an abstract variable, a sayable half,
    and a stack slot with an address.
    """
    from qbopt import omf
    from qbopt import module
    from qbopt import blocks as split
    from qbopt.rewrite import rewrite
    from qbopt.rewrite import code_map

    assert REWRITTEN.exists(), "the bench object is what this test is about"
    result = rewrite(REWRITTEN.read_bytes(), dry_run=False)
    out = result[0] if isinstance(result, tuple) else result
    found = module.of(omf.parse(out))
    assert found is not None
    mapped = code_map(found)
    assert not isinstance(mapped, str), mapped
    total = sum(len(simplify.round_trips(body)) for _, body in mir.bodies(found, split.partition(found, mapped)))
    assert total == 0, "the pass ran; what it found should be gone from its own output"


@pytest.mark.parametrize("obj", FIXTURES, ids=lambda p: p.stem)
def test_a_slot_is_only_ever_compared_within_its_block(obj: Path) -> None:
    """A Space.STACK disp is a depth from the top of its own block."""
    _, raised = bodies(obj)
    for _, body in raised:
        for block in body.blocks:
            for op in block.ops:
                for ref in op.loads + op.stores:
                    if ref.addr is not None and ref.addr.space is Space.STACK:
                        assert ref.addr.base == mir.Register.NONE, "a stack slot is named by depth alone"
