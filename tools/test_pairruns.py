"""A rows pair names the tests/run outputs a flip changed."""
from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import pairruns  # noqa: E402

BASE = "[c] 350 programs: 350 pass, 0 known, 0 fail\nFAIL  examples/planets.nib: line 5: want 'a', got 'b'\n"


def test_a_flip_that_changes_one_output_is_reported_with_the_test_and_the_level():
    """JumpThreading on by default printed 30921 for 30192 in logical_value.c at -O2 and -Os (m16 and m32); the pair showed clocks and bytes only."""
    new = BASE + "FAIL  c/logical_value.c [-O2]: line 1: want '30192', got '30921'\nFAIL  c/logical_value.c [-Os] [x86-m32]: line 1: want '30192', got '30921'\n"
    added, fixed = pairruns.compare(BASE, new)
    assert added == [
        "NEW FAIL c/logical_value.c [-O2]: line 1: want '30192', got '30921'",
        "NEW FAIL c/logical_value.c [-Os] [x86-m32]: line 1: want '30192', got '30921'",
    ] and fixed == []


def test_a_failure_both_sides_have_is_not_the_flips_and_a_fixed_one_is_named():
    added, fixed = pairruns.compare(BASE, "[c] 350 programs: 350 pass, 0 known, 0 fail\n")
    assert added == [] and fixed == ["examples/planets.nib"]
    assert pairruns.compare(BASE, BASE) == ([], [])


def test_a_base_that_fails_hundreds_of_programs_is_a_broken_premise_not_no_new_failure():
    """Both sides failed 493 programs ('jwasm: not found': the pair's dist dir lacked the tools) and the diff read 'no new tests/run failure' for #2907."""
    flood = "[c] 493 programs: 0 pass, 0 known, 493 fail\n" + "".join(f"FAIL  c/p{i}.c: link: jwasm: not found\n" for i in range(493))
    added, fixed = pairruns.compare(flood, flood)
    assert len(added) == 1 and added[0].startswith("PREMISE BROKEN: the base fails 493 programs") and "jwasm" in added[0] and fixed == []


def test_a_new_side_that_fails_hundreds_of_programs_is_a_broken_premise_too():
    """The new side of #2948 and #2902 lacked jwasm (493 'not found' failures) while the base was clean: the guard looked at the base only and printed 493 NEW FAIL lines."""
    flood = "[c] 493 programs: 0 pass, 0 known, 493 fail\n" + "".join(f"FAIL  c/p{i}.c: link: jwasm: not found\n" for i in range(493))
    added, fixed = pairruns.compare("[c] 350 programs: 350 pass, 0 known, 0 fail\n", flood)
    assert len(added) == 1 and added[0].startswith("PREMISE BROKEN: the new fails 493 programs") and fixed == []


def test_a_side_that_ran_nothing_is_a_broken_premise_not_no_new_failure():
    """The base of #2948 and #2902 printed one 'stale binaries' line and no group: the diff said 'no new tests/run failure' of a base that never ran."""
    ran = "[c] 350 programs: 350 pass, 0 known, 0 fail\n"
    added, _ = pairruns.compare("stale binaries in /x: llrm-cc older than the source\ndone\n", ran)
    assert len(added) == 1 and added[0].startswith("PREMISE BROKEN: the base run printed no group line") and "stale binaries" in added[0]


def test_a_tree_at_another_sha_than_the_binaries_is_refused(tmp_path):
    """planets.nib failed on both sides of a pair (want 10.0375, got 10.037499) because the tests came from another commit than the binaries; it read as a failure of the base."""
    import subprocess

    def git(*args):
        return subprocess.run(["git", "-C", str(tmp_path), "-c", "user.name=t", "-c", "user.email=t@t", *args], capture_output=True, text=True, check=True).stdout.strip()

    git("init", "-q")
    git("commit", "-q", "--allow-empty", "-m", "a")
    first = git("rev-parse", "HEAD")
    git("commit", "-q", "--allow-empty", "-m", "b")
    second = git("rev-parse", "HEAD")
    assert pairruns.tree_mismatch(tmp_path, second) == ""
    assert pairruns.tree_mismatch(tmp_path, second[:9]) == ""
    assert "TREE MISMATCH" in pairruns.tree_mismatch(tmp_path, first)
