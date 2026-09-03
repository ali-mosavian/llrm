"""
tools/opportunity.py's own gate: the board has to answer for our output.

It costed the parsed object for a while, which is BC's code and not ours.
Every ratio on the board then sat perfectly still no matter what a pass
did -- a reading that agreed with "there is nothing here" and was believed
for exactly as long as it took to notice hotlop had not moved after the
hoist started firing on it.

That is rule 2 in the project's own words, and the instrument that broke is
the one every other number is judged by, so it gets a test of its own.
"""

import sys
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).resolve().parent.parent / "tools"))

import opportunity


# Programs the passes provably change. Any of them would do; more than one
# so that a single fixture going quiet cannot make this vacuous.
MOVED = ("hotlop-p-g2", "press-p-g2", "spill-p-g2")


@pytest.mark.parametrize("name", MOVED)
def test_the_board_costs_our_output_and_not_bc(name: str) -> None:
    """--raw is BC. The default is what we ship, and they must differ."""
    path = Path(f"fixtures/omf/{name}.obj")
    ours = opportunity.counted([path])["cost"]
    theirs = opportunity.counted([path], raw=True)["cost"]
    assert ours and theirs, f"{name}: no cost at all, so this proves nothing"
    assert ours < theirs, (
        f"{name}: costed {ours} against BC's {theirs} -- the board is not "
        "reading our output, or the passes stopped paying"
    )


# Targets for programs no fixture exists for. They score nothing and cannot,
# so they are written down here rather than found again: the point of the
# test below is that this list does not grow without someone saying so.
UNBUILT = {"HG", "FX"}


def test_a_target_is_a_number_the_board_can_reach() -> None:
    """Every target names a program that exists, and none is zero.

    A target for a program with no fixture never prints and never fails --
    it simply is not on the board -- so nothing else would ever say so.
    """
    named = {opportunity._program(path) for path in Path("fixtures/omf").glob("*.obj")}
    for program, want in opportunity.TARGETS.items():
        assert want > 0, f"{program}: a target of {want} makes every ratio infinite"
    dead = {program for program in opportunity.TARGETS if program not in named}
    assert dead == UNBUILT, f"targets naming no fixture: {sorted(dead)}, expected {sorted(UNBUILT)}"
