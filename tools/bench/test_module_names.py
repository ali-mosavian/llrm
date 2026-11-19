import collections
import sys
from pathlib import Path

import bench  # noqa: F401  (its imports put the harness directories on sys.path)

ROOT = Path(__file__).resolve().parents[2]


def test_no_two_modules_on_the_harness_path_share_a_name():
    """tools/bench/clocks.py and vsgcc/clocks.py were both `clocks`: in one pytest process whichever imported first answered for both, and 16 flat-target
    and BASIC tests failed with `module 'clocks' has no attribute 'prefixes'` (each file passed alone)."""
    seen = collections.defaultdict(set)
    for entry in {Path(one).resolve() for one in sys.path if one}:
        if ROOT in entry.parents and entry.is_dir():
            for file in entry.glob("*.py"):
                if not file.name.startswith("test_") and file.name != "conftest.py":
                    seen[file.stem].add(file.relative_to(ROOT).as_posix())
    assert {name: sorted(paths) for name, paths in seen.items() if len(paths) > 1} == {}
