"""The m32 C leg's compare step: the bench's rule on two vsgcc runs."""
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import compare  # noqa: E402


def row(variant, ins=10, mem=4, clocks=50, code=100, ok=True):
    return {"prog": "p", "variant": variant, "ins": ins, "mem": mem, "clocks": clocks, "code": code, "data": 0, "ok": ok}


def run(main_rows, tip_rows):
    return compare.judge({(r["prog"], r["variant"]): r for r in main_rows}, {(r["prog"], r["variant"]): r for r in tip_rows})


def test_the_gate_rule_applies_per_level_to_llrm_cells():
    """-O2 is priced (+13 B for -12 clocks pays at 312 mc/B), -Os is strict on bytes, -O1 is reported only."""
    refusals, fails, judged, reported = run(
        [row("llrm"), row("llrmOs"), row("llrmO1")],
        [row("llrm", code=113, clocks=38), row("llrmOs", code=101, clocks=40), row("llrmO1", code=999)],
    )
    assert not refusals and (judged, reported) == (2, 1)
    assert fails == ["p m32 -Os: code_bytes 101 > 100 (worse)"]


def test_a_cell_set_that_differs_or_a_row_that_did_not_run_is_refused_not_passed():
    refusals, _, _, _ = run([row("llrm"), row("llrmOs")], [row("llrm")])
    assert any("cell sets differ" in line for line in refusals)
    refusals, _, _, _ = run([row("llrm")], [row("llrm", ins=0)])
    assert any("no instructions" in line for line in refusals)
