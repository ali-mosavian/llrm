"""sizesweep's summary counts the rows of a sweep and never the TOTAL line as one."""
from __future__ import annotations

import importlib.util
import unittest
from pathlib import Path

spec = importlib.util.spec_from_file_location("sizesweep", Path(__file__).resolve().parent / "sizesweep.py")
sizesweep = importlib.util.module_from_spec(spec)
spec.loader.exec_module(sizesweep)

SWEEP = "c/a.c [216, 115]\nc/b.c [160, 100]\nqb/c.bas [100, 110]\nTOTAL [1000, 949]\n"


class SummarizeTests(unittest.TestCase):
    def test_the_total_line_is_not_a_row(self):
        """The first summariser matched `TOTAL [..]` as a row: 32 rows shrinking 1942 B were reported as 3884 B in a PR body."""
        summary = sizesweep.summarize(SWEEP)
        self.assertIn("2 shrink by 161 B", summary)
        self.assertNotIn("212", summary)

    def test_a_grower_and_the_total_are_reported(self):
        summary = sizesweep.summarize(SWEEP)
        self.assertIn("1 variants grow by 10 B,", summary)
        self.assertIn("total 1000 -> 949 (-51 B)", summary)


if __name__ == "__main__":
    unittest.main()


def test_two_sweeps_do_not_share_an_object_path(monkeypatch):
    """sizesweep wrote every object to /tmp/sizesweep.obj: two sweeps at once (or another session) read each other's object and printed another's size."""
    import subprocess
    import sys
    import types

    paths = []
    program = types.SimpleNamespace(source=Path("a.c"), flags=[], dialect="")
    monkeypatch.setitem(sys.modules, "run_tests", types.SimpleNamespace(discover=lambda args: [program]))
    monkeypatch.setitem(sys.modules, "sizes", types.SimpleNamespace(code_bytes=lambda obj: 10))

    def fake_run(command, **kwargs):
        obj = Path(command[command.index("-o") + 1])
        paths.append(obj)
        obj.write_bytes(b"x")

    monkeypatch.setattr(subprocess, "run", fake_run)
    sizesweep.sweep("/before", "/after", "dag", "-Os")
    sizesweep.sweep("/before", "/after", "dag", "-Os")
    assert Path("/tmp/sizesweep.obj") not in paths and paths[0] != paths[2], "each sweep has an object path of its own"
    assert not any(path.exists() for path in paths), "the scratch directory is removed"
