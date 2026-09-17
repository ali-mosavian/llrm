"""
qbopt/legacy/simplify.py's own gate.

A deletion that is wrong is silent, so what is tested here is not that the
round trips are found but that nothing else is: the identity has to hold
for the same reason every time, and the register at both ends has to agree.
"""

from pathlib import Path

import pytest

import corpus
from iced_x86 import Register

from qbopt.model import ir
from qbopt.model import mir
from qbopt.legacy import simplify
from qbopt.objectfile.module import Space

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
        assert simplify.round_trips(body, raised.hints[body.entry]) == ()


@pytest.mark.parametrize("obj", FIXTURES, ids=lambda p: p.stem)
def test_every_round_trip_names_one_register_at_both_ends(obj: Path) -> None:
    """What makes the deletion safe rather than merely correct.

    Removing the idiom leaves the register holding the value, so every later
    instruction finds what it expected. A rejoin landing somewhere else
    would need a `mov` emitting, and is refused instead.
    """
    _, raised = bodies(obj)
    for _, body in raised:
        hints = raised.hints[body.entry]
        for trip in simplify.round_trips(body, hints):
            assert hints.origin_of(trip.value) is trip.source
            assert trip.free == (trip.source is trip.target)


def test_a_real_round_trip_is_found_and_proved() -> None:
    """bench/nbody.bas, after absorption: the worked example in the module.

    v152 = concat(high16(v150), low16(v150)) = v150, which needs all three
    of docs/variables.md's stages -- an abstract variable, a sayable half,
    and a stack slot with an address.
    """
    from qbopt.objectfile import omf
    from qbopt.objectfile import module
    from qbopt.frontend import blocks as split
    from qbopt.rewrite import rewrite
    from qbopt.frontend.blocks import code_map

    assert REWRITTEN.exists(), "the bench object is what this test is about"
    result = rewrite(REWRITTEN.read_bytes(), dry_run=False)
    out = result[0] if isinstance(result, tuple) else result
    found = module.of(omf.parse(out))
    assert found is not None
    mapped = code_map(found)
    assert not isinstance(mapped, str), mapped
    raised = mir.bodies(found, split.partition(found, mapped))
    left = [trip for _, body in raised for trip in simplify.round_trips(body, raised.hints[body.entry])]
    # A free one needs nothing emitted, so none may survive the pass that
    # removes them. One landing in another register needs a move, and is
    # refused until something emits it.
    assert not [one for one in left if one.free], "a free round trip survived"


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


def test_a_round_trip_whose_register_is_overwritten_is_refused() -> None:
    """The guard, on the shape that made a real program print the wrong number.

    A generated program's `x MOD y MOD z` came out of absorption as

        push bx / push cx      the outer divisor, split into halves
        ...                    an absorbed MOD, ending in
        pop ecx                its own divisor
        idiv ecx
        pop ecx                the rejoin
        idiv ecx               which wanted the value from the pushes

    Source and target are both ecx, so the trip looked free and all three
    ops were deleted -- and the second idiv divided by the first one's
    divisor. 25375 where the answer was 8734.

    Tested here rather than through a program because a small one will not
    reproduce it: suite/chain.bas has the nested divide and the round trips,
    and BC keeps the value somewhere else. It took the register pressure of
    a forty-statement generated program to bite, which is why the
    integration evidence is tools/fuzzcheck.py and this is the unit.
    """
    ecx = Register.ECX
    value = mir.Value(1, 0x100)
    body = mir.MirBody(
        entry=0x100,
        blocks=(
            mir.MirBlock(
                at=0x100,
                phis=(),
                ops=(
                    mir.Op(at=0x100, op=ir.Operation.PUSH, name="push", defines=(), uses=()),
                    mir.Op(at=0x107, op=ir.Operation.POP, name="pop", defines=(value,), uses=()),
                    mir.Op(at=0x111, op=ir.Operation.POP, name="pop", defines=(), uses=()),
                ),
                succ=(),
            ),
        ),
    )
    hints = mir.AllocationHints(origins={value.variable: ecx})
    block = body.blocks[0]
    assert not simplify._target_survives(body, hints, block, 0x100, 0x111, ecx), (
        "the pop at 0x107 overwrote ecx"
    )
    # and the same span with nothing writing it is still allowed
    assert simplify._target_survives(body, hints, block, 0x100, 0x111, Register.EBX)


def test_every_suite_program_fits_a_dos_file_name() -> None:
    """`B_` and `O_` go in front of the name, and DOS keeps eight characters.

    A seven-letter program becomes B_CHAINED, DOS writes B_CHAINE, and the
    redirect in RUN.BAT lands somewhere the harness never looks: every
    configuration reports "the baseline produced no output" for a program
    that compiled without a warning and ran to the end.
    """
    for program in sorted(Path("suite").glob("*.bas")):
        assert len(program.stem) <= 6, f"{program.name}: B_{program.stem.upper()} is more than eight characters"
