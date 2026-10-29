"""The parity smoke's counter test: it reads hardware counters, so the pytest-perf step runs it where the host has them (REQUIRES) and a hosted gate reports it missing by name."""
from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import parity_smoke as ps  # noqa: E402

REQUIRES = ["perf"]


def test_a_cut_step_reports_the_instructions_it_ran(tmp_path, monkeypatch):
    """The budget read 0 G for a unit step that run_cut killed and ~120 G for one that finished (llrm-codegen, twice): SIGKILL took perf's counts with it."""
    monkeypatch.undo()  # the real Meter: the autouse fixture swaps in a stub
    with ps.Meter() as counted:
        code, _ = ps.run_cut([sys.executable, "-c", "while True: pass"], 1.0, tmp_path)
    assert code is None
    assert counted.value is not None and counted.value > 100_000_000, counted.value
