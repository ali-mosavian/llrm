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
    sizesweep.sweep("/before", "/after", "dag", "-Os", "m16")
    sizesweep.sweep("/before", "/after", "dag", "-Os", "m16")
    assert Path("/tmp/sizesweep.obj") not in paths and paths[0] != paths[2], "each sweep has an object path of its own"
    assert not any(path.exists() for path in paths), "the scratch directory is removed"


def _fake_sweep(monkeypatch, flags_per_program, target):
    import subprocess
    import sys
    import types

    commands = []
    programs = [types.SimpleNamespace(source=Path(f"p{n}.c"), flags=flags, dialect="") for n, flags in enumerate(flags_per_program)]
    monkeypatch.setitem(sys.modules, "run_tests", types.SimpleNamespace(discover=lambda args: programs))
    monkeypatch.setitem(sys.modules, "sizes", types.SimpleNamespace(code_bytes=lambda obj: 10))

    def fake_run(command, **kwargs):
        commands.append(command)
        Path(command[command.index("-o") + 1]).write_bytes(b"x")

    monkeypatch.setattr(subprocess, "run", fake_run)
    return sizesweep.sweep("/before", "/after", "dag", "-Os", target), commands


def test_a_sweep_labelled_m16_compiles_every_program_at_m16_and_leaves_the_m32_variants_out(monkeypatch):
    """sizesweep took run_tests.discover whole: 650 default-target and 411 `-m32` variants of 1061 in one TOTAL, cited as "m16 -Os" by seven PRs."""
    lines, commands = _fake_sweep(monkeypatch, [[], ["-m32"], ["-m16"]], "m16")
    assert all("-m16" in command and "-m32" not in command for command in commands) and len(commands) == 4, "two programs, two dists each"
    assert lines[0].startswith("# target m16: 2 programs compiled with -m16 forced, 1 variants of the other target left out")
    assert lines[-1] == "TOTAL [20, 20]", "the m32 variant is not in the total"


def test_a_sweep_labelled_m32_measures_only_the_m32_variants(monkeypatch):
    lines, commands = _fake_sweep(monkeypatch, [[], ["-m32"]], "m32")
    assert len(commands) == 2 and all("-m32" in command and "-m16" not in command for command in commands)
    assert lines[0].startswith("# target m32: 1 programs")


def test_a_command_that_would_compile_at_the_other_target_is_refused():
    import pytest

    with pytest.raises(SystemExit, match="labelled m16 but would compile at -m32"):
        sizesweep.require_target(["llrm-c", "a.c", "-m16", "-m32"], "m16")
    with pytest.raises(SystemExit, match="default target"):
        sizesweep.require_target(["llrm-c", "a.c"], "m16")
    sizesweep.require_target(["llrm-c", "a.c", "-m16"], "m16")
