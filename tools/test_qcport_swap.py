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
