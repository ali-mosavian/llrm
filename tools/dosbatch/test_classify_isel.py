"""classify_isel puts every FAIL line in one cause row, and refuses what no rule names."""

import sys
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).resolve().parent))
import classify_isel  # noqa: E402

LOG = """\
stamp x
FAIL  c/a.c [-O2 -m32]: compile: llrm-c: @f_: --isel=dag: CannotSelect("MergeValues")
FAIL  c/b.c [-O2 -m32]: compile: llrm-c: @g_: --isel=dag: CannotSelect("MergeValues")
FAIL  c/c.c [-Os -m32]: compile: llrm-c: @h_: --isel=dag: Emit(Unsupported("frame index -2"))
FAIL  c/d.c: link: llrm-c: llrm-c: @i_: --isel=dag: Emit(Unsupported("frame index 6"))
FAIL  c/e.c [-O2 -m32]: line 2: want '12', got '21'
FAIL  c/f.c [-O2 -m32]: link: jwlink: Error! E2028: div64_s32 is an undefined reference
FAIL  c/g.c [-O2 -m32]: compile: llrm-c: @j_: --isel=dag: some new builder refusal
449 programs: 442 pass, 0 known, 7 fail
"""


def test_every_failure_is_in_exactly_one_row_and_the_rows_sum():
    rows, unclassified = classify_isel.classify(LOG)
    assert unclassified == []
    assert sum(len(programs) for programs in rows.values()) == 7
    assert rows[("unselected node MergeValues", "sdag-c")] == ["c/a.c [-O2 -m32]", "c/b.c [-O2 -m32]"]
    assert len(rows[("emit: frame index N", "sdag-d")]) == 2
    assert rows[("wrong output", "triage")] == ["c/e.c [-O2 -m32]"]
    assert ("builder refusal: some new builder refusal", "sdag-a") in rows
    assert classify_isel.table(LOG).splitlines()[-1] == "| total | | 7 | |"


def test_a_failure_no_rule_names_is_refused():
    with pytest.raises(SystemExit, match="unclassified"):
        classify_isel.table(LOG + "FAIL  c/z.c: something nobody anticipated\n")
