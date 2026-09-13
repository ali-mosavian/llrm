"""A loop proven to run once is entered at its body, and keeps what its exit reads."""

from pathlib import Path

from qbopt import wholeseg

FIXTURE = Path("fixtures/regressions/qbdemo-fil2.obj")


def test_entering_mains_first_loop_at_its_body_keeps_the_code_after_it() -> None:
    """Re-deriving the moved phis by variable renamed main's exit copy of a
    call's answer to the loop counter; decide folded the exit away and the
    segment was refused."""
    result = wholeseg.emitted(FIXTURE.read_bytes())
    assert result.outcome is wholeseg.Emission.LIR, result.reason


def test_a_back_edge_keeps_its_copies_in_the_latch() -> None:
    """Entered at its body, harr's inner loop branched to a copy block placed
    after the procedure and jumped back: two jumps a pass. The copies define
    only what the loop reads, so they belong before the branch."""
    body = {}

    def watch(stage, name, state):
        if stage == "peephole":
            body[name] = state

    result = wholeseg.emitted(Path("fixtures/omf/harr-v-g3.obj").read_bytes(), watch=watch)
    assert result.outcome is wholeseg.Emission.LIR, result.reason
    split = [
        f"{name} {block.at:#x}"
        for name, state in body.items()
        for block in state.blocks
        if block.at >= (state.entry + 1) << 32
    ]
    assert not split, split
