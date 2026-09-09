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


@pytest.mark.parametrize("tag", ["p-evt", "q-evt", "v-evt"])
def test_event_build_does_not_use_a_plain_program_target(tag, tmp_path, capsys):
    """BOOLS /V/W was scored against a reference with no event checks (QB read as 4.57x)."""
    path = tmp_path / "bools-renamed.obj"
    path.write_bytes(Path(f"fixtures/omf/bools-{tag}.obj").read_bytes())
    assert opportunity.against_targets([path], raw=True) != 0
    report = capsys.readouterr().out
    assert "PROVISIONAL" in report and "event" in report
    assert "x " not in report


def test_event_configuration_cannot_pass_even_below_plain_target(monkeypatch, capsys):
    from collections import Counter
    monkeypatch.setattr(opportunity, "counted", lambda *args: Counter({"cost": 1, "event-enabled configuration": 1}))
    assert opportunity.against_targets([Path("bools-q-O.obj")]) != 0
    assert "PROVISIONAL" in capsys.readouterr().out


def test_a_fallback_is_not_scored_as_success(monkeypatch) -> None:
    """A backend refusal could score BC or fallback bytes as optimized output."""
    monkeypatch.setattr(opportunity.rewrite, "rewrite", lambda data, **kwargs: (data, []))
    with pytest.raises(opportunity.Unmeasured, match="LIR"):
        opportunity._measured(Path("fixtures/omf/hotlop-p-g2.obj"), raw=False)


def test_unmapped_output_is_not_a_zero_cost_success(monkeypatch, capsys) -> None:
    """bools-q-O was reported as 0.0x when the output could not be mapped."""
    monkeypatch.setattr(opportunity, "code_map", lambda _: "unmapped code")
    status = opportunity.against_targets([Path("fixtures/omf/bools-q-O.obj")])
    report = capsys.readouterr().out
    assert status != 0
    assert "UNMEASURED" in report and "0.0x" not in report


def test_above_target_is_not_success(monkeypatch) -> None:
    """A target report returned success even when press exceeded 1.5x."""
    from collections import Counter

    monkeypatch.setattr(opportunity, "counted", lambda *args: Counter(cost=463))
    assert opportunity.against_targets([Path("press-p-g2.obj")]) != 0


def test_rewrite_failure_cannot_be_scored_as_optimized_output(monkeypatch) -> None:
    """A failed rewrite silently scored BC's original hotlop as our output."""

    def broken(*args, **kwargs):
        raise RuntimeError("rewrite failed")

    monkeypatch.setattr(opportunity.rewrite, "rewrite", broken)
    path = Path("fixtures/omf/hotlop-p-g2.obj")
    with pytest.raises(RuntimeError, match="rewrite failed"):
        opportunity._measured(path, raw=False)
    assert opportunity._measured(path, raw=True) is not None


def test_cost_does_not_make_unknown_memory_accesses_free() -> None:
    """harr's cost changed when allocation obscured an array's address.

    The instruction still accesses memory if its exact address is unknown.
    Changing only address knowledge must not change its execution cost.
    """
    from collections import Counter
    from dataclasses import replace

    from qbopt.model import mir
    from qbopt.objectfile import omf
    from qbopt.objectfile import module
    from qbopt.frontend import blocks as split

    found = module.of(omf.parse(Path("fixtures/omf/harr-p-g2.obj").read_bytes()))
    blocks = split.partition(found, split.code_map(found))
    before, after = Counter(), Counter()
    for _name, body in mir.bodies(found, blocks):
        obscured = replace(
            body,
            blocks=tuple(
                replace(
                    block,
                    ops=tuple(
                        replace(
                            op,
                            loads=tuple(replace(ref, addr=None) for ref in op.loads),
                            stores=tuple(replace(ref, addr=None) for ref in op.stores),
                        )
                        for op in block.ops
                    ),
                )
                for block in body.blocks
            ),
        )
        opportunity._cost(body, found, before)
        opportunity._cost(obscured, found, after)
    assert before["cost"] > 0
    assert after["cost"] == before["cost"], "unknown addresses made real memory accesses free"


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


def test_missing_target_cannot_verify_completion(monkeypatch, capsys):
    """Nbody had no hand-derived target, yet its target report returned success."""
    from collections import Counter
    monkeypatch.setattr(opportunity, "counted", lambda *args: Counter(cost=1))
    assert opportunity.against_targets([Path("nbody-p-g2.obj")]) != 0
    assert "NO TARGET" in capsys.readouterr().out


@pytest.mark.parametrize("program", ["pressx", "hotlpx", "lngmxx", "fpcse", "fpcsex"])
def test_provisional_target_cannot_verify_completion(program, monkeypatch, capsys):
    """Runtime-input twins inherited constant-source targets; FP references changed rounding and sums."""
    from collections import Counter
    monkeypatch.setattr(opportunity, "counted", lambda *args: Counter(cost=1))
    assert opportunity.against_targets([Path(f"{program}-p-g2.obj")]) != 0
    assert "PROVISIONAL" in capsys.readouterr().out
