"""A cell with the same bytes and more instructions or clocks is worse. The old
bytes-only awk comparator printed nothing for it (m64 round 5, frames: bytes +4,
instrs +4.6 %; a clocks-only row has equal bytes)."""
import os
import sys
sys.path.insert(0, os.path.dirname(__file__))
import cmpcells  # noqa: E402

BASE = {"c O2": [100.0, 1000.0, 50.0]}
NEW = {"c O2": [100.0, 1100.0, 50.0]}
CLK = {"c O2": [100.0, 1000.0, 60.0]}


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
    assert cmpcells.worse(BASE, {"c O2": [99.0, 900.0, 49.0]}) == []
    assert cmpcells.worse(BASE, BASE) == []
