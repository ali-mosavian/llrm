"""tools/scoreboard.py: a row whose compile failed is in no total. gcc on QCport (Open Watcom C) failed on 57 of 65 files, and the
first version of the table summed what it counted before it stopped: 'gcc spends 90% of its compile parsing', 'llrm is 10x gcc'."""
import importlib.util
import shutil
import subprocess
import sys
from pathlib import Path

import pytest

HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location("scoreboard", HERE / "scoreboard.py")
scoreboard = importlib.util.module_from_spec(spec)
sys.modules["scoreboard"] = scoreboard
spec.loader.exec_module(scoreboard)


def run(name, llrm, gcc, parse=100, level="-O2", set="programs"):
    return {"file": name, "set": set, "level": level, "llrm": llrm, "gcc": gcc, "gcc_parse": parse, "rows": {"mir gvn": 1.0}}


def test_a_compile_that_fails_gives_no_count():
    if shutil.which("perf") is None or subprocess.run(["perf", "stat", "-e", "instructions:u", "-o", "/dev/null", "--", "true"], capture_output=True).returncode:
        pytest.skip("no perf counters")  # a runner without them fails perf itself, before the compile runs
    count, why = scoreboard.instructions(["python3", "-c", "import sys; sys.stderr.write('boom'); sys.exit(1)"], ok=True)
    assert count is None and "boom" in why


def test_a_row_gcc_failed_in_is_refused_and_in_no_total():
    runs = [run("good", 3_000_000, 2_000_000), run("bad", 9_000_000, None, parse=None)]
    report = scoreboard.report(runs)
    assert scoreboard.refused(runs) == ["bad -O2: gcc failed"]
    assert "REFUSED 1 rows" in report and "bad -O2: gcc failed" in report
    assert "llrm 3.0 Minstr, gcc 2.0" in report, report  # the failed row's 9 M are nowhere
    assert [r["file"] for r in scoreboard.clean(runs)] == ["good"]


def test_a_row_llrm_failed_in_is_refused_too():
    runs = [run("good", 3_000_000, 2_000_000), run("bad", None, 5_000_000)]
    assert scoreboard.refused(runs) == ["bad -O2: llrm failed"]
    assert "gcc 2.0" in scoreboard.report(runs)
