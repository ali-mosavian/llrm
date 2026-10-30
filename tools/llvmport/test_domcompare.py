"""`uv run --project tools python -m pytest tools/llvmport/test_domcompare.py`: the comparison fails what it is for."""
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from domcompare import first_difference  # noqa: E402


def test_a_changed_idom_a_missing_block_and_a_missing_tree_are_differences():
    same = ["@f", "dt 0 - 0 1,", "dt 1 0 1 ", "dt roots 0,"]
    assert first_difference(same, list(same)) is None
    assert first_difference([*same[:1], "dt 0 - 0 2,", *same[2:]], same) == ("dt 0 - 0 2,", "dt 0 - 0 1,")
    assert first_difference(same[:3], same) == ("", "dt roots 0,")
    assert first_difference([], same) == ("", "@f")
