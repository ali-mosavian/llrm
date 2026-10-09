"""pipeline-runs.py reads llrm-c's `runs`, `time` and `regalloc` lines: a reader that drops a line type reports a quiet compile."""
import importlib.util
from pathlib import Path

HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location("pipeline_runs", HERE / "pipeline-runs.py")
pr = importlib.util.module_from_spec(spec)
spec.loader.exec_module(pr)

SAMPLE = """\
[runs] step r01-fold changed 1000000
[runs] step r02-fold idle 3000000
[runs] step r02-gvn idle 1000000
[runs] body 1 trigger "": 1 fixed points, 2 rounds, 3 pass runs, 0 skipped as settled, 1 changes, work idle 4000000 useful 1000000
[runs] body 1 trigger "inline0.": 1 fixed points, 1 rounds, 5 pass runs, 2 skipped as settled, 0 changes, work idle 0 useful 0
[regalloc] _f@3: 40 insns, 2 spilled, cost 10, 0 forced
[instr]       12.000 Minstr own       15.000 Minstr total       3x assemble
[instr] total 20.000 Minstr, outermost steps 19.000 Minstr, outside every step 1.000 Minstr
"""


def test_every_line_type_is_read():
    got = pr.parsed(SAMPLE)
    assert got["bodies"]["1"]["first"] == [1, 2, 3] and got["bodies"]["1"]["inline."] == [1, 1, 5]
    assert got["steps"] == [("r01-fold", "changed", 1000000), ("r02-fold", "idle", 3000000), ("r02-gvn", "idle", 1000000)]
    assert got["total"] == 20e6 and got["spans"]["assemble"] == (15e6, 3) and got["allocated"] == 1


def test_the_report_bills_idle_work_by_round_and_pass():
    text = pr.report([pr.parsed(SAMPLE)])
    assert "pipeline runs per body: median 2" in text
    assert "idle by round (5: five and later): r2 4.0 G" not in text and "r2 0.0 G" in text
    assert "fold 0.0 G (75% of its work)" in text and "first 37.5%" in text and "inline. 62.5%" in text


def test_a_compile_with_instruction_counts_has_a_total_and_one_without_has_none():
    """The `time` header names Mcpu-ns on every host, so a search for the word called every compile counterless and printed SKIPPED."""
    header = "[instr] by own work, in Minstr (the thread's user-space instructions; Mcpu-ns where the host has no counter):\n"
    assert pr.parsed(header + SAMPLE)["total"] == 20e6
    assert pr.parsed(header + "[instr] total 5.0 Mcpu-ns\n")["total"] == 0.0
