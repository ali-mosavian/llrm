import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
import rebase_clocks as rc  # noqa: E402


def test_a_clock_bar_is_main_clocks_under_the_same_judge_not_a_kept_number():
    """The m16 multiplier fix moved main's own clocks (40 of 342 cells) and the 48 hand-kept limits stayed the old judge's."""
    text = '[clocks]\n# class 0\n"tile bas -Os" = 10277  # main 10277\n"gone c -Os" = 5\n'
    out, missing = rc.rebased(text, {"tile/bas/Os": {"clocks": 10300}})
    assert '"tile bas -Os" = 10300  # main 10300' in out and '"gone c -Os" = 5' in out and "# class 0" in out
    assert missing == ["gone c"]
