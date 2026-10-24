"""objsweep's split of the changed objects: larger, smaller and same-size-different-bytes, per target and level."""
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
import objsweep


def test_changed_objects_split_per_target_and_level_with_the_largest_growths():
    """A sweep printed 110 changed objects and no split: the reader hand-parsed the list and got it wrong."""
    changed = [("a m32 -O1", 100, 90), ("b m32 -O1", 100, 130), ("c m16 -Os", 50, 50), ("d m16 -Os", 10, 12), ("e m32 -O2", 7, 8)]
    lines = objsweep.summary(changed, top=2)
    assert "m32 -O1: larger 1, smaller 1, same-size 0" in lines
    assert "m16 -Os: larger 1, smaller 0, same-size 1" in lines
    assert lines.index("largest growths:") < lines.index("  b m32 -O1: +30")
    assert "  d m16 -Os: +2" in lines and "  e m32 -O2: +1" not in lines
