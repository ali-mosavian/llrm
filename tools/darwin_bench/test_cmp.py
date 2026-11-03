"""A cell that regresses on one metric alone must be flagged (novec.py ignored insns, so an instruction-only regression printed nothing)."""
from cmp import regressions, regressions_vs_best, clock_note


def test_each_metric_alone():
    ref = (100, 30, 10.0)
    assert regressions((104, 30, 10.0), ref) == ["bytes"]
    assert regressions((100, 31, 10.0), ref) == ["insns"]
    assert regressions((100, 30, 12.0), ref) == ["us"]


def test_clean():
    assert regressions((100, 30, 10.5), (100, 30, 10.0)) == []


def test_best_of_takes_min_per_metric():
    assert regressions_vs_best((100, 30, 10.0), [(90, 40, 10.0), (120, 25, 10.0)]) == ["bytes", "insns"]


def test_clock_note_marks_noisy_above_ten_percent():
    """Every clocks cell was printed as plain 'us' although spreads reach 10-300% on the loaded Mac."""
    assert clock_note(2.0, [10.0, 10.5, 10.2]) == "darwin us, load 2.0, spread 5%"
    assert clock_note(2.0, [10.0, 12.0, 10.2]) == "darwin us, load 2.0, spread 20%, noisy"
    assert clock_note(2.0, [10.0, 10.1, 10.2], ok=False).endswith("output differs from .out")
