"""The hourly sweep: what it reads from the runners' output, which reds are new, and the line it prints."""
from __future__ import annotations

import subprocess
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import sweep_hourly as s  # noqa: E402

GROUPS = """stamp /x: abc
[alias] 4 programs: 4 pass, 0 known, 0 fail

[c] 393 programs: 392 pass, 0 known, 1 fail
FAIL  bitfield_long_long: t.lo differs

[huge] 44 programs: 40 pass, 0 known, 4 fail  LAUNCH STOPPED (4 programs unjudged)

[scalar] scalar: no summary (rc 1)
"""


def test_a_failing_program_is_a_wrong_output_and_a_stopped_launch_or_a_missing_summary_is_red():
    reds, count = s.parse_groups(GROUPS)
    names = {one.name: one.kind for one in reds}
    assert names["run:bitfield_long_long"] == s.WRONG
    assert names["run:huge:launch-stopped"] == s.OTHER
    assert names["run:scalar:no-summary"] == s.OTHER
    assert count == "436/441"


def test_a_group_summary_with_no_failure_has_no_red():
    reds, count = s.parse_groups("[alias] 4 programs: 4 pass, 0 known, 0 fail\n")
    assert reds == [] and count == "4/4"


def test_qbrt_notpass_and_compile_fail_are_red_and_the_result_line_is_the_count():
    text = "PROGRAMS 2 a b\nCOMPILE-FAIL a -O2: x\nNOTPASS b m32 ok ok differs\nRESULT 1 of 2 pass (program x target); 1 did not compile\n"
    reds, count = s.parse_qbrt(text)
    assert {(one.name, one.kind) for one in reds} == {("qbrt:a:compile", s.COMPILE), ("qbrt:b:m32", s.WRONG)}
    assert count == "1/2"


def test_a_new_qcport_kind_is_red_and_a_known_one_is_not():
    text = (
        "QCport sweep of abc: 65 modules x 4 levels + 466 front-end compiles (39 m16-only), 3 failed compiles, 2 kinds, 1 new\n"
        "  KNOWN #2454: x -- menu -O2\n  NEW: llrm-c: a byte division -- vid -Os\n"
    )
    reds, count = s.parse_qcport(text)
    assert [one.name for one in reds] == ["qcport:llrm-c: a byte division -- vid -Os"]
    assert count == "3 failed/726"


def test_a_failed_demo_is_a_wrong_output():
    text = "FAILED tools/dosbatch/test_qbdemos.py::test_the_demo_shows[GORILLA] - AssertionError\n1 failed, 1 passed in 400s\n"
    reds, count = s.parse_demos(text)
    assert [(one.name, one.kind) for one in reds] == [("demo:GORILLA", s.WRONG)]
    assert count == "1/2"


def test_only_a_red_the_previous_run_did_not_have_is_new():
    now = [s.Red("run:a", s.WRONG), s.Red("run:b", s.WRONG)]
    assert [one.name for one in s.newly_red(now, {"run:a": s.WRONG})] == ["run:b"]
    assert s.newly_red(now, {"run:a": s.WRONG, "run:b": s.WRONG}) == []


def test_the_pull_request_is_the_number_in_the_squash_subject():
    assert s.pull_request("fix(x): a thing (#3681)") == "#3681"
    assert s.pull_request("a subject with no number at all and more words past forty chars").startswith("a subject")


def test_the_merge_range_is_the_first_parent_commits_after_the_previous_tip(tmp_path):
    def git(*args):
        return subprocess.run(["git", *args], cwd=tmp_path, capture_output=True, text=True, check=True).stdout.strip()

    git("init", "-q")
    git("config", "user.email", "t@t")
    git("config", "user.name", "t")
    shas = []
    for n in range(3):
        (tmp_path / "f").write_text(str(n))
        git("add", "f")
        git("commit", "-q", "-m", f"change {n} (#{100 + n})")
        shas.append(git("rev-parse", "HEAD"))
    got = s.merges(shas[0], shas[2], tmp_path)
    assert [s.pull_request(subject) for _, subject in got] == ["#101", "#102"]
    assert [s.pull_request(subject) for _, subject in s.merges(None, shas[2], tmp_path)] == ["#102"]


def test_the_bisect_finds_the_first_merge_that_is_red_in_log_steps():
    range_ = [(f"sha{i}", f"m{i} (#{i})") for i in range(8)]
    asked = []

    def asks_red(sha):
        asked.append(sha)
        return int(sha[3:]) >= 5

    found = s.bisect(range_, asks_red, budget=6)
    assert found is not None and found[0] == 5 and len(asked) <= 3


def test_the_bisect_gives_up_when_the_budget_of_builds_is_spent():
    range_ = [(f"sha{i}", f"m{i} (#{i})") for i in range(8)]
    assert s.bisect(range_, lambda sha: int(sha[3:]) >= 5, budget=1) is None
    red = s.Red("run:x", s.WRONG, ["run_tests.py", "x"])
    assert s.first_bad(red, range_, 1, lambda red, sha: int(sha[3:]) >= 5) == "one of " + ", ".join(f"#{i}" for i in range(8))


def test_a_single_merge_in_the_range_is_the_one_that_made_the_red():
    red = s.Red("run:x", s.WRONG)
    assert s.first_bad(red, [("a", "fix (#7)")], 6) == "#7"


def test_the_line_names_a_new_wrong_output_before_the_other_reds_and_prints_the_stamp():
    new = [s.Red("qcport:k", s.COMPILE), s.Red("run:p", s.WRONG)]
    out = s.line("a" * 40, "b" * 40, "llrm-c=12345678", {"run": "10/10"}, new, new, {"run:p": "#9", "qcport:k": "#8"})
    assert out.startswith("sweep aaaaaaaaa RED 2 stamp=bbbbbbbbb md5 llrm-c=12345678 | run 10/10 | new: WRONG OUTPUT run:p (#9), qcport:k (#8)")


def test_a_clean_run_is_pass_with_nothing_new():
    out = s.line("a" * 40, "a" * 40, "m", {"run": "1/1"}, [], [], {})
    assert " PASS " in out and out.endswith("new: none")


def test_the_state_is_the_previous_run_and_a_missing_file_is_an_empty_one(tmp_path):
    path = tmp_path / "state.json"
    assert s.load_state(path) == {"tip": None, "reds": {}}
    s.save_state(path, "abc", [s.Red("run:a", s.WRONG)])
    assert s.load_state(path) == {"tip": "abc", "reds": {"run:a": s.WRONG}}
