"""A cell with the same bytes and more instructions or clocks is worse. The old
bytes-only awk comparator printed nothing for it (m64 round 5, frames: bytes +4,
instrs +4.6 %; a clocks-only row has equal bytes)."""
import os
import sys
sys.path.insert(0, os.path.dirname(__file__))
import cmpcells  # noqa: E402

BASE = {"c O2": [100.0, 100000.0, 500.0]}
NEW = {"c O2": [100.0, 110000.0, 500.0]}
CLK = {"c O2": [100.0, 100000.0, 600.0]}


def old_awk(base, new):
    # the pre-fix comparator: bytes only
    return [c for c, (b, _, _) in new.items() if b > base[c][0]]


def test_instrs_only_regression_is_flagged():
    assert old_awk(BASE, NEW) == []
    assert [c for c, _ in cmpcells.worse(BASE, NEW)] == ["c O2"]


def test_clocks_only_regression_is_flagged():
    assert old_awk(BASE, CLK) == []
    assert [c for c, _ in cmpcells.worse(BASE, CLK)] == ["c O2"]


def test_equal_and_better_are_not_flagged():
    assert cmpcells.worse(BASE, {"c O2": [99.0, 90000.0, 490.0]}) == []
    assert cmpcells.worse(BASE, BASE) == []


def test_thirty_more_instructions_on_a_small_cell_are_worse():
    assert [c for c, _ in cmpcells.worse({"k O2": [50.0, 200.0, 1.0]}, {"k O2": [50.0, 230.0, 1.0]})] == ["k O2"]


def test_an_unchanged_row_is_clean():
    assert cmpcells.worse({"k O2": [50.0, 200.0, 500.0]}, {"k O2": [50.0, 200.0, 500.0]}) == []


def test_a_short_run_moving_in_ms_is_not_flagged():
    assert cmpcells.worse({"c": [10.0, 9000.0, 1.0]}, {"c": [10.0, 9000.0, 43.0]}) == []
