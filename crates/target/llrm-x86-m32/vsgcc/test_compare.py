"""The m32 C leg's compare step: the bench's rule on two vsgcc runs."""
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import compare  # noqa: E402


def row(variant, ins=10, mem=4, clocks=50, code=100, ok=True, cc=None):
    return {"prog": "p", "variant": variant, "ins": ins, "mem": mem, "clocks": clocks, "code": code, "data": 0, "ok": ok, **({"cc": cc} if cc else {})}


def keyed(rows, cc):
    return {(r["prog"], r["variant"]): {"cc": cc, **r} for r in rows}


def run(main_rows, tip_rows):
    """Main built by one compiler, tip by another, unless a row says otherwise."""
    return compare.judge(keyed(main_rows, "aa+bb"), keyed(tip_rows, "cc+bb"))


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


def test_a_file_of_two_compilers_or_a_pair_of_one_is_refused():
    """The A/B that measured a stale llrm-llc on both sides reported 0 differing cells: a file names its compiler, and main and tip must name two."""
    refusals, _, _, _ = run([row("llrm"), row("llrmOs", cc="dd+bb")], [row("llrm"), row("llrmOs")])
    assert any("two compilers" in line for line in refusals)
    refusals, _, _, _ = compare.judge(keyed([row("llrm")], "aa+bb"), keyed([row("llrm")], "aa+bb"))
    assert any("same compiler" in line for line in refusals)
    refusals, _, _, _ = compare.judge({("p", "llrm"): row("llrm")}, keyed([row("llrm")], "cc+bb"))
    assert any("no compiler stamp" in line for line in refusals)
