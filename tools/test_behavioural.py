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
    assert b.touches(["crates/backend/llrm-core/src/backend/coalesce.rs"])
    assert b.touches(["crates/backend/llrm-core/src/liveness/liverangecalc.rs", "docs/x.md"])
    assert b.touches(["crates/target/llrm-x86/src/isel/peephole.peep"]), "a peephole rule changes the code"
    # #4146 changed these and no earlier path rule named either: the tier skipped it
    assert b.touches(["crates/backend/llrm-core/src/backend/machinesched.rs", "crates/backend/llrm-core/src/flow.rs"])
    assert not b.touches(["docs/working/regalloc-plan.md", "crates/backend/llrm-core/src/backend/coalesce_tests.rs", "tools/parity_squash.py"])
    assert not b.touches(["crates/backend/llrm-core/src/backend/selectiondag/emit.rs"]), "the DAG path is not the default compile"


def test_the_red_programs_are_read_from_run_tests_output():
    assert b.red_programs(RED) == ["regparm3_matrix_22", "regparm3_matrix_31", "regparm3_matrix_4"]
    assert b.red_programs(GREEN) == []


def test_a_batch_with_4146s_files_is_refused_and_a_docs_batch_builds_nothing():
    """#4146 changed machinesched.rs and flow.rs and landed red on regparm3_matrix_22/_31/_4; a batch of only docs or DAG-shadow files must not pay a build."""
    import parity_squash as ps

    built = []
    red = ps.pre_merge_behavioural(["crates/backend/llrm-core/src/backend/machinesched.rs", "crates/backend/llrm-core/src/flow.rs"], build=lambda: built.append(1), run=lambda: RED)
    assert red == ["regparm3_matrix_22", "regparm3_matrix_31", "regparm3_matrix_4"] and built
    built.clear()
    assert ps.pre_merge_behavioural(["docs/working/x.md", "crates/backend/llrm-core/src/backend/selectiondag/emit.rs"], build=lambda: built.append(1), run=lambda: RED) == [] and not built


def test_the_offender_is_the_one_backend_pr_or_the_one_that_is_red_alone():
    import parity_squash as ps

    files = {"4146": ["crates/backend/llrm-core/src/backend/machinesched.rs"], "4170": ["docs/a.md"], "4171": ["crates/backend/llrm-core/src/backend/coalesce.rs"]}
    probed = []
    assert ps.behavioural_culprits(["4146", "4170"], files.__getitem__, lambda n: probed.append(n) or True) == ["4146"] and not probed, "one backend PR: it is the offender, no build"
    assert ps.behavioural_culprits(["4146", "4170", "4171"], files.__getitem__, lambda n: n == "4146") == ["4146"], "two backend PRs: each merged alone and asked"
