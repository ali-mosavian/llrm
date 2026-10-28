"""The behavioural tier: a merge touching the allocator, coalescer, liveness, selector, peephole or frame is followed by a run of the call-shaped programs, and a wrong one is a red line."""
from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import behavioural as b  # noqa: E402

# What run_tests.py printed on the tree of #4152 (f73d0874e0), 2026-10-23, and on its parent.
RED = """FAIL  c/regparm3_matrix_22.c [-O2]: line 1: want '33853', got '0'
FAIL  c/regparm3_matrix_31.c [-O2]: line 13: want '-1099918129', got '0'
FAIL  c/regparm3_matrix_4.c [-O2]: line 7: want '653', got '2552'
3 programs: 0 pass, 0 known, 3 fail
"""
GREEN = "3 programs: 3 pass, 0 known, 0 fail\n"


def test_a_tree_with_the_4152_bug_turns_the_tier_red_and_its_parent_does_not(monkeypatch):
    """The 22:00 sweep was green and #4152 landed 22:42; the 23:00 sweep found three wrong outputs. The tier names them as soon as the bins are built."""
    monkeypatch.setattr(b, "merges", lambda old, new: [("f73d0874e0aa", "feat(coalescer): a dead copy goes through eliminateDeadDefs", True)])
    lines = b.check("f73d0874e0aa", "bc0e5e9f79", build=lambda tip: RED)
    assert [line.rsplit(" ", 1)[1] for line in lines] == ["regparm3_matrix_22", "regparm3_matrix_31", "regparm3_matrix_4"]
    assert all(line.startswith("BEHAVIOURAL RED: f73d0874e feat(coalescer)") for line in lines), lines
    assert b.check("bc0e5e9f79", "x", build=lambda tip: GREEN) == []


def test_a_range_of_merges_that_touch_no_tier_path_runs_nothing(monkeypatch):
    monkeypatch.setattr(b, "merges", lambda old, new: [("aaa", "docs(x): y", False), ("bbb", "feat(opt): z", False)])
    ran = []
    assert b.check("bbb", "ooo", build=lambda tip: ran.append(tip) or RED) == [] and not ran


def test_the_paths_are_code_of_the_allocator_coalescer_liveness_selector_peephole_and_frame():
    assert b.touches(["crates/codegen/llrm-codegen/src/backend/coalesce.rs"])
    assert b.touches(["crates/codegen/llrm-codegen/src/liveness/liverangecalc.rs", "docs/x.md"])
    assert b.touches(["crates/target/llrm-x86/src/isel/peephole.peep"]), "a peephole rule changes the code"
    # #4146 changed these and no earlier path rule named either: the tier skipped it
    assert b.touches(["crates/codegen/llrm-codegen/src/backend/machinesched.rs", "crates/codegen/llrm-codegen/src/flow.rs"])
    assert not b.touches(["docs/working/regalloc-plan.md", "crates/codegen/llrm-codegen/src/backend/coalesce_tests.rs", "tools/parity_squash.py"])
    assert not b.touches(["crates/codegen/llrm-codegen/src/backend/selectiondag/emit.rs"]), "the DAG path is not the default compile"


def test_the_red_programs_are_read_from_run_tests_output():
    assert b.red_programs(RED) == ["regparm3_matrix_22", "regparm3_matrix_31", "regparm3_matrix_4"]
    assert b.red_programs(GREEN) == []


def test_a_batch_with_4146s_files_is_refused_and_a_docs_batch_builds_nothing():
    """#4146 changed machinesched.rs and flow.rs and landed red on regparm3_matrix_22/_31/_4; a batch of only docs or DAG-shadow files must not pay a build."""
    import parity_squash as ps

    built = []
    red = ps.pre_merge_behavioural(["crates/codegen/llrm-codegen/src/backend/machinesched.rs", "crates/codegen/llrm-codegen/src/flow.rs"], build=lambda: built.append(1), run=lambda: RED)
    assert red == ["regparm3_matrix_22", "regparm3_matrix_31", "regparm3_matrix_4"] and built
    built.clear()
    assert ps.pre_merge_behavioural(["docs/working/x.md", "crates/codegen/llrm-codegen/src/backend/selectiondag/emit.rs"], build=lambda: built.append(1), run=lambda: RED) == [] and not built


def test_the_offender_is_the_one_backend_pr_or_the_one_that_is_red_alone():
    import parity_squash as ps

    files = {"4146": ["crates/codegen/llrm-codegen/src/backend/machinesched.rs"], "4170": ["docs/a.md"], "4171": ["crates/codegen/llrm-codegen/src/backend/coalesce.rs"]}
    probed = []
    assert ps.behavioural_culprits(["4146", "4170"], files.__getitem__, lambda n: probed.append(n) or True) == ["4146"] and probed == ["4146"], "one backend PR: blamed only after it is run alone"
    assert ps.behavioural_culprits(["4146", "4170"], files.__getitem__, lambda n: False) == [], "green alone: the batch's red was not its"
    assert ps.behavioural_culprits(["4146", "4170", "4171"], files.__getitem__, lambda n: n == "4146") == ["4146"], "two backend PRs: each merged alone and asked"


def test_a_red_among_many_backend_prs_is_found_by_halving_not_one_by_one():
    """The 23:53 batch tested its backend members alone at ~2 min each; a 48-PR batch would take 48 runs. Halving finds the one red member in about 2 log2(n)."""
    import parity_squash as ps

    members = [str(n) for n in range(5000, 5048)]
    files = {n: ["crates/codegen/llrm-codegen/src/backend/machinesched.rs"] for n in members}
    asked = []
    alone = []

    def red_with(subset):
        asked.append(len(subset))
        return "5031" in subset

    assert ps.behavioural_culprits(members, files.__getitem__, lambda n: alone.append(n) or False, red_with) == ["5031"]
    assert len(asked) <= 2 * 6 + 1 and not alone, "about 2 log2(48) runs, no alone runs"


def test_halving_that_finds_no_red_half_falls_back_to_alone():
    import parity_squash as ps

    files = {"1": ["crates/codegen/llrm-codegen/src/backend/a.rs"], "2": ["crates/codegen/llrm-codegen/src/backend/b.rs"]}
    assert ps.behavioural_culprits(["1", "2"], files.__getitem__, lambda n: n == "2", lambda subset: False) == ["2"], "an interaction: no subset is red, each is asked alone"


def test_a_red_integration_test_among_28_members_is_found_in_a_dozen_runs_not_28():
    """The 00:34 batch named #5563 for target_facts after 21 minutes of serial alone runs (28 members, ~45 s each); halving names it in about 2 log2(28) runs."""
    import parity_squash as ps

    members = [str(n) for n in range(5500, 5528)]
    asked = []
    alone = []

    def red_with(subset):
        asked.append(len(subset))
        return "5507" in subset

    assert ps.red_members(members, red_with, lambda n: alone.append(n) or False) == ["5507"]
    assert len(asked) <= 2 * 5 + 1 and not alone, asked


# What run_tests.py printed for PROGRAMS on the tree of #4146 (a1d904596), 2026-10-24: 10 of 68 configurations, the 10 the 01:00 sweep counted (1004/1014).
FOUR_ONE_FOUR_SIX = """FAIL  c/memfuncs_runtime.c [-O2 -march=i486 -m32 -mabi=sysv]: line 1: want '0', got '56'
FAIL  c/regparm3_matrix_22.c: line 13: want '45840', got '11208'
FAIL  c/regparm3_matrix_31.c: line 3: want '-49', got '96'
FAIL  c/regparm3_matrix_4.c: line 7: want '653', got '2552'
FAIL  qb/bounds.bas: line 1: want 'local 4905  9  6  5', got 'error 9'
FAIL  qb/range_power.bas [-O1]: line 16: want '-8  .3333333333333333 error 5', got '-8  .3333333333333333 ok'
FAIL  qb/range_power.bas [-Os]: line 16: want '-8  .3333333333333333 error 5', got '-8  .3333333333333333 ok'
FAIL  examples/channels.nib: line 2: want 'open b', got 'open a'
FAIL  examples/level.nib: line 1: want '3 planes 3 nodes 5 faces, 62 bytes', got 'LEVEL.DAT is not a level'
FAIL  grep/grep.bas: line 1: want '4745', got 'Subscript out of range in line No line number in module GREP
68 programs: 58 pass, 0 known, 10 fail
"""


def test_the_set_covers_all_ten_wrong_outputs_of_4146_across_frontends():
    """The 21 C programs saw 3 of the 10; the QB, Nib and memfuncs programs saw the rest."""
    wrong = b.red_programs(FOUR_ONE_FOUR_SIX)
    assert wrong == ["bounds", "channels", "grep", "level", "memfuncs_runtime", "range_power", "regparm3_matrix_22", "regparm3_matrix_31", "regparm3_matrix_4"]
    assert set(wrong) <= set(b.PROGRAMS)
    assert FOUR_ONE_FOUR_SIX.count("\nFAIL") + 1 == 10


def test_a_red_that_is_green_on_the_second_run_refuses_nothing():
    """#5785 was refused for 23 wrong programs 100 s after the same tree ran 0 wrong (LLRM_OW missing in one run): one red run is no proof."""
    import parity_squash as ps

    runs = iter([RED, GREEN])
    assert ps.pre_merge_behavioural(["crates/codegen/llrm-codegen/src/backend/machinesched.rs"], build=lambda: None, run=lambda: next(runs)) == []


def test_a_red_both_times_names_a_missing_open_watcom_tree(monkeypatch, capsys):
    import parity_squash as ps

    monkeypatch.delenv("LLRM_OW", raising=False)
    assert ps.pre_merge_behavioural(["crates/codegen/llrm-codegen/src/backend/machinesched.rs"], build=lambda: None, run=lambda: RED)
    assert "LLRM_OW" in capsys.readouterr().out
