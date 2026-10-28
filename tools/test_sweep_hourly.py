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
        "  KNOWN #2454: x -- menu -O2\n  NEW: llrm-c: a byte division -- vid -Os\n  NEW: panic x.rs:N:N: lsr added a memory operation -- matrix.bas -m32 -O1, idioms.bas -m16 -O2\n"
    )
    reds, count = s.parse_qcport(text)
    assert [one.name for one in reds][0] == "qcport:llrm-c: a byte division -- vid -Os"
    assert reds[0].probe == [], "a QCport C module is a file outside the tree: it cannot be asked at another build"
    assert reds[1].probe == ["qcport_probe.py", "matrix.bas", "-m32", "-O1", "panic x.rs:N:N: lsr added a memory operation"]
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


def test_a_qcport_kind_is_asked_by_its_first_program_and_a_missing_job_is_not_called_green(tmp_path, monkeypatch):
    """The 12 compile panics of 4548930fb named a kind and no probe: the bisect gave up with the whole range. The probe exit 3 (no such job) is `None`, not green."""
    calls = []

    def run(argv, **kwargs):
        calls.append(argv)
        return type("Done", (), {"returncode": 3 if "gone.bas" in argv else 1, "stdout": "", "stderr": ""})()

    binaries = tmp_path / "sbp-abcdefghi" / "release"
    binaries.mkdir(parents=True)
    (binaries / "llrm-llc").write_text("")
    monkeypatch.setattr(s, "SCRATCH", tmp_path)
    monkeypatch.setattr(s.subprocess, "run", run)
    assert s.ask_at(s.Red("qcport:k", s.COMPILE, ["qcport_probe.py", "matrix.bas", "-m32", "-O1", "panic"]), "abcdefghi" + "0" * 31) is True
    assert s.ask_at(s.Red("qcport:k", s.COMPILE, ["qcport_probe.py", "gone.bas", "-m32", "-O1", "panic"]), "abcdefghi" + "0" * 31) is None
    assert calls[0][1].endswith("tools/qcport_probe.py")


def test_a_qbrt_probe_is_given_its_work_directory_and_a_usage_error_is_not_red(tmp_path, monkeypatch):
    """The 15:00 sweep blamed a docs-only PR (#4021) for 35 wrong outputs: the qbrt probe ran `qbrt_run.py --only NAME` with no work directory, which is a usage
    error (exit 2), every probe read red, and the bisect named the oldest merge of the range."""
    calls = []

    def run(argv, **kwargs):
        calls.append(argv)
        return type("Done", (), {"returncode": 2 if argv[2].startswith("--") else 0, "stdout": "RESULT 1 of 1 pass", "stderr": ""})()

    binaries = tmp_path / "sbp-abcdefghi" / "release"
    binaries.mkdir(parents=True)
    (binaries / "llrm-llc").write_text("")
    monkeypatch.setattr(s, "SCRATCH", tmp_path)
    monkeypatch.setattr(s.subprocess, "run", run)
    red = s.Red("qbrt:arena_type:x86-m32", s.WRONG, ["dosbatch/qbrt_run.py", "--only", "arena_type"])
    assert s.ask_at(red, "abcdefghi" + "0" * 31) is False, "a passing probe is green"
    assert not calls[0][2].startswith("--") and calls[0][3:] == ["--only", "arena_type"]


def test_a_program_that_did_not_link_is_a_build_failure_not_wrong_output():
    """qbrt 183/218 on 50387f114 was `jwlink: memcpy_ is an undefined reference` for 35 programs, reported as 35 WRONG OUTPUTs."""
    text = "NOTPASS arena_type x86-m32 ok not built jwlink: Error! E2028: memcpy_ is an undefined reference\nNOTPASS bc_draw x86-m16 ok DIFF 3 lines\nRESULT 216 of 218 pass (program x target); 0 did not compile\n"
    reds, _ = s.parse_qbrt(text)
    assert [(one.name, one.kind) for one in reds] == [("qbrt:arena_type:x86-m32", s.BUILD), ("qbrt:bc_draw:x86-m16", s.WRONG)]
    report = s.line("t" * 40, "t" * 40, "", {}, reds, reds, {})
    assert "BUILD FAILURE qbrt:arena_type:x86-m32" in report and "WRONG OUTPUT qbrt:bc_draw:x86-m16" in report
    assert "WRONG OUTPUT qbrt:arena_type" not in report


def test_a_qbrt_step_that_died_before_its_count_is_a_red_with_the_reason():
    """The 01:00 sweep of 2026-10-24 printed `qbrt ?` for a qbrt_run that died with "RuntimeError: DOSBox ran 19 of 20 jobs": the line read as unknown and the step as no red."""
    text = "Traceback (most recent call last):\n  File x\nRuntimeError: DOSBox ran 19 of 20 jobs (/p/events.txt)\n"
    reds, result = s.parse_qbrt(text)
    assert result == "?" and [one.name for one in reds] == ["qbrt:no-result: RuntimeError: DOSBox ran 19 of 20 jobs (/p/events.txt)"]
    assert s.parse_qbrt("RESULT 218 of 218 pass (program x target); 0 did not compile\n") == ([], "218/218")


def test_a_sweep_records_its_pid_for_the_loop_that_waits_for_it(tmp_path, monkeypatch):
    """The loop waited with `pgrep -f '^python3 tools/sweep_hourly'`, which also matches any session whose command line starts that way; it waits on this pid now."""
    import os

    seen = []
    monkeypatch.setattr(s, "RUNNING", tmp_path / "running.pid")
    monkeypatch.setattr(s, "sweep", lambda argv: seen.append((tmp_path / "running.pid").read_text().strip()) or 0)
    assert s.main([]) == 0
    assert seen == [str(os.getpid())] and not (tmp_path / "running.pid").exists()


def test_a_program_that_did_not_link_is_a_build_failure_not_a_wrong_output():
    """The 23:00 sweep of 2026-10-24 called `FAIL c/bench_io.c [-O2 -march=i486]: link: jwlink: Error! E2028: _llrm_os_close@3 is an undefined reference` a WRONG OUTPUT, and #4370 was blamed for a file race that was never reached."""
    text = "[c] 430 programs: 429 pass, 0 known, 1 fail\nFAIL  c/bench_io.c [-O2 -march=i486]: link: jwlink: Error! E2028: _llrm_os_close@3 is an undefined reference\n"
    reds, _ = s.parse_groups(text)
    assert [(one.name, one.kind) for one in reds] == [("run:c/bench_io.c", s.BUILD)]
    reds, _ = s.parse_groups("[c] 1 programs: 0 pass, 0 known, 1 fail\nFAIL  c/x.c [-O2]: compile: llrm-c: boom\n")
    assert reds[0].kind == s.BUILD
    reds, _ = s.parse_groups("[c] 1 programs: 0 pass, 0 known, 1 fail\nFAIL  c/x.c [-O2]: output differs at line 3\n")
    assert reds[0].kind == s.WRONG


def test_a_sweep_step_runs_under_a_build_slot():
    """The sweep's test steps (run_groups, os_objects, llrmbin) ran beside four cargo builds at load 68 on 32 cores; every step now holds one of the shared slots."""
    seen = []
    real = s.subprocess.run
    s.subprocess.run = lambda argv, **kw: seen.append(argv) or subprocess.CompletedProcess(argv, 0, "", "")
    try:
        import tempfile

        s.run(["python3", "step.py"], {}, Path(tempfile.mkdtemp()) / "log", Path("."))
    finally:
        s.subprocess.run = real
    assert Path(seen[0][seen[0].index("python3") - 1]).name == "buildslot" and seen[0][-2:] == ["python3", "step.py"]


def test_a_red_unrun_test_target_is_a_red_of_the_sweep():
    """greedy_inline sat red on parity for a day: the landing gate runs lib tests and scanners only, and no step of the sweep ran tests/*.rs."""
    reds, count = s.parse_inventory("llrm/greedy_inline: 0.4 s, RED\nllrm/nearcalls: 0.0 s, ok\ninventory: 2 unrun targets run, 1 red\n")
    assert [red.name for red in reds] == ["unrun:llrm/greedy_inline"] and count == "1/2"
