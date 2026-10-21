"""`uv run --project tools python -m pytest tools/test_qcport_swap.py`"""
import importlib.util
import sys
from pathlib import Path

spec = importlib.util.spec_from_file_location("qcport_swap", Path(__file__).parent / "qcport-swap.py")
qcport_swap = importlib.util.module_from_spec(spec)
sys.modules["qcport_swap"] = qcport_swap
spec.loader.exec_module(qcport_swap)


def arm(name, exe="a", instructions=100, frames="61", polys="37022", obj="o"):
    return {"name": name, "exe": exe, "obj": obj, "instructions": instructions, "frames": frames, "polys": polys}


def test_two_runs_of_the_same_exe_that_count_differently_void_the_comparison():
    """An instrument whose repeat run differs measures the host, not the module: the base arm is run twice for this."""
    problems = qcport_swap.verdict([arm("base"), arm("base-again", instructions=101)])
    assert problems and "same EXE" in problems[0]


def test_a_repeat_of_the_base_that_moves_the_count_is_noise_but_a_path_sized_move_is_not():
    """The run directory's name changes the count by about 100 of 485,835,518; a repeat 1% off is the host."""
    assert qcport_swap.verdict([arm("base", exe="a", instructions=485_835_457), arm("base-again", exe="b", instructions=485_835_560)]) == []
    problems = qcport_swap.verdict([arm("base", exe="a", instructions=485_835_457), arm("base-again", exe="b", instructions=490_000_000)])
    assert problems and "noise" in problems[0]


def test_arms_that_draw_different_frames_are_not_compared():
    """A 61-frame run and a 60-frame run did different work; their counts differ by the work, not the code."""
    problems = qcport_swap.verdict([arm("base"), arm("head", exe="b", instructions=90, frames="60")])
    assert problems and "different work" in problems[0]


def test_different_exes_that_draw_the_same_work_compare():
    assert qcport_swap.verdict([arm("base"), arm("head", exe="b", instructions=90)]) == []


def test_the_report_names_each_arms_binaries_and_its_difference_from_the_first():
    text = qcport_swap.report([arm("base", exe="1234567890"), arm("head", exe="abcdefghij", instructions=130)])
    assert "+30" in text and "12345678" in text and "abcdefgh" in text


def test_the_end_event_is_the_count_and_a_run_without_one_is_refused():
    assert qcport_swap.instructions_of('{"ev":"end","instructions":3}\n{"ev":"end","instructions":7}\n') == 7
    try:
        qcport_swap.instructions_of('{"ev":"end","instructions":3}\n')
    except RuntimeError:
        return
    raise AssertionError("no end event accepted")


def test_two_identical_runs_have_no_function_above_their_own_noise():
    """EMS paging order moved QGLGEMMAP +7.4M and QGLGEMFREE -7.4M between runs of byte-identical objects: a per-function
    delta below that is not a finding."""
    base = {"QGLGEMMAP": 20_000_000, "QGLGEMFREE": 9_000_000, "_d_draw_faces@3": 24_000_000}
    again = {"QGLGEMMAP": 27_400_000, "QGLGEMFREE": 1_600_000, "_d_draw_faces@3": 24_000_100}
    rises, falls = qcport_swap.noise(base, again)
    assert (rises, falls) == (7_400_100, 7_400_000)
    floor = max(abs(one) for one in qcport_swap.deltas(base, again).values())
    assert qcport_swap.flagged(base, again, floor) == {}


def test_a_change_above_the_noise_is_flagged_and_one_below_it_is_not():
    base = {"QGLGEMMAP": 20_000_000, "_d_draw_faces@3": 24_000_000, "_clip_w@3": 100_000}
    arm = {"QGLGEMMAP": 20_100_000, "_d_draw_faces@3": 33_000_000, "_clip_w@3": 150_000}
    assert qcport_swap.flagged(base, arm, 7_400_000) == {"_d_draw_faces@3": 9_000_000}


def test_a_rise_and_a_matching_fall_are_paged_work_not_a_finding():
    """w-4's identical-object runs moved QGLGEMMAP +7.4M and QGLGEMFREE -7.4M: flagged above any floor of one repeat, but a pair."""
    changes = {"QGLGEMMAP": 7_400_000, "QGLGEMFREE": -7_350_000, "_d_draw_faces@3": 2_400_000}
    assert qcport_swap.paired(changes) == {"QGLGEMMAP", "QGLGEMFREE"}


def test_the_repeat_arms_are_all_named_base_again_and_each_is_checked():
    arms = [dict(name="base", exe="a", obj="o", instructions=485_835_457, frames="1", polys="1"),
            dict(name="base-again1", exe="b", obj="o", instructions=485_835_560, frames="1", polys="1"),
            dict(name="base-again3", exe="d", obj="o", instructions=490_000_000, frames="1", polys="1")]
    problems = qcport_swap.verdict(arms)
    assert problems and "base-again3" in problems[0]
