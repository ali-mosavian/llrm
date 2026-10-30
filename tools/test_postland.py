from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import postland  # noqa: E402


def test_a_failure_the_previous_head_or_the_xfail_list_has_is_not_new():
    """A loop that named every standing failure as the batch's would open an issue per head."""
    listed = {"unit llrm-qb::a": {}}
    assert postland.new_failures({"run x", "run y", "unit llrm-qb::a"}, {"run x"}, listed) == {"run y"}


def test_the_culprit_is_the_first_commit_that_fails_and_none_when_the_head_does_not():
    commits = list("abcdefgh")
    assert postland.culprit(commits, lambda sha: sha >= "e") == "e"
    assert postland.culprit(commits, lambda sha: sha >= "a") == "a"
    assert postland.culprit(commits, lambda sha: False) is None


def test_the_culprit_search_runs_log_n_probes():
    seen = []
    postland.culprit(list(map(str, range(16))), lambda sha: seen.append(sha) or int(sha) >= 11)
    assert len(seen) <= 5


def test_a_squash_subject_names_its_pr_and_a_body_its_owner():
    assert postland.pr_of("fix(gate): the sigterm test waits (#1755)") == 1755
    assert postland.pr_of("merge: origin/release/next") is None
    assert postland.owner_of("TL;DR: x\n\nOwner: bench-instruments-2206\n") == "bench-instruments-2206"
    assert postland.owner_of("no owner here") == "unknown"


def test_a_failure_has_a_command_that_runs_it_alone():
    assert postland.repro("run c/idioms.c [-O2]") == "python3 tools/dosbatch/run_tests.py idioms"
    assert postland.repro("unit llrm-qb::mod::test_x") == "cargo test --release -p llrm-qb --lib mod::test_x"


def test_the_report_is_a_table_of_failure_culprit_owner_repro():
    rows = [{"failure": "run scroll", "commit": "abc1234567", "pr": 2890, "owner": "simplify-combine-wiring-2128", "repro": "python3 tools/dosbatch/run_tests.py scroll"}]
    text = postland.report("f" * 40, "e" * 40, rows, ["tests/run: 57 groups"])
    assert "| run scroll | #2890 | simplify-combine-wiring-2128 | `python3 tools/dosbatch/run_tests.py scroll` |" in text
    assert "No new failure" in postland.report("f" * 40, "e" * 40, [], ["tests/run: 57 groups"])


def test_failing_tests_are_attributed_to_the_crate_whose_binary_ran_them():
    """Per-crate runs lost the root package's features: 19 llrm-c tests failed with 'built without the toolchain feature' and were reported as the batch's."""
    out = (
        "     Running unittests src/lib.rs (target/release/deps/llrm_c-0123abcd)\n\nfailures:\n    compile::tests::test_a\n\ntest result: FAILED. 1 failed\n"
        "     Running unittests src/lib.rs (target/release/deps/llrm_mir-4567ef01)\n\ntest result: ok. 3 passed\n"
        "     Running unittests src/lib.rs (target/release/deps/llrm_qb-89abcdef)\n\nfailures:\n    t::test_b\n    test_c\n\ntest result: FAILED. 2 failed\n"
    )
    assert postland.by_binary(out) == {"llrm-c": ["compile::tests::test_a"], "llrm-qb": ["t::test_b", "test_c"]}


def test_a_failed_run_that_names_no_test_is_a_failure_not_a_pass():
    """`cargo test -q` printed no `Running` lines, the parse found nothing, and 59 untested landings were reported 'No new failure'."""
    assert postland.unit_result("error: could not compile `llrm-c`\n", 101) == {"unit workspace::failed-without-a-test-name"}
    assert postland.unit_result("test result: ok. 3 passed\n", 0) == set()


def test_stderr_and_stdout_keep_their_order(tmp_path):
    """Cargo's `Running` (stderr) must stay before its crate's results (stdout); read apart, 14 failing tests had no crate."""
    script = "echo 'Running unittests (deps/llrm_c-0a1b)' >&2; echo 'failures:'; echo '    t::x'"
    output, code = postland.merged(["sh", "-c", script], tmp_path, {})
    assert code == 0 and output.index("Running") < output.index("failures:")


def test_a_tree_built_after_a_newer_one_is_still_newer_than_the_shared_targets_artifacts(tmp_path):
    """A shared target called the older commit's sources fresh and linked the newer commit's crate: 10 compile errors at the head, and a culprit (#2757) that was not."""
    import os
    import subprocess

    def git(*args):
        subprocess.run(["git", "-C", str(tmp_path), "-c", "user.name=t", "-c", "user.email=t@t", *args], capture_output=True, check=True)

    git("init", "-q")
    (tmp_path / "a.rs").write_text("x")
    git("add", "a.rs")
    git("commit", "-q", "-m", "a")
    artifact = tmp_path / "artifact"
    artifact.write_text("built later")
    os.utime(tmp_path / "a.rs", (1, 1))
    postland.fresh(tmp_path)
    assert (tmp_path / "a.rs").stat().st_mtime >= artifact.stat().st_mtime


def test_the_report_opens_with_what_ran_so_a_short_list_is_not_read_as_health():
    """A report of two failures said nothing of the 40 suites that never ran."""
    text = postland.report("f" * 40, "e" * 40, [], ["tests/run, both widths: 57 groups, 858 programs, 0 failing", "NOT RUN: fmt.sh"])
    assert text.index("COVERED SUITES") < text.index("No new failure") and "NOT RUN: fmt.sh" in text


def test_the_summaries_count_the_groups_and_the_crates_that_ran_and_say_so_when_none_did():
    assert postland.group_summary("[c] 351 programs: 349 pass, 0 known, 2 fail\n[nib] 49 programs: 49 pass, 0 known, 0 fail\n") == "2 groups, 400 programs, 2 failing"
    assert postland.group_summary("") == "NOT RUN (no group line)"
    out = "     Running unittests src/lib.rs (deps/llrm_c-0a1b)\ntest result: FAILED. 140 passed; 2 failed; 0 ignored\n"
    assert postland.unit_summary(out) == "llrm-c 142/2" and postland.unit_summary("error") == "NOT RUN (no test result)"


def test_the_partial_report_lists_what_ran_and_each_raw_failure_line():
    """The list existed only in memory for 1 h; the owners could not be told while the bisect ran."""
    text = postland.partial("f" * 40, "e" * 40, ["tests/run: 57 groups"], ["run c/x.c [-O2]: want '1', got '2'"])
    assert "COVERED SUITES" in text and "want '1', got '2'" in text and "bisect pending" in text


def test_the_premise_line_names_a_program_combine_changed_and_says_when_the_binary_is_not_the_build(tmp_path, monkeypatch):
    """Today's instruments passed silently three times: a 0-failing run is only a result if the tested binary is the build and combine changed something."""
    monkeypatch.setattr(postland, "HOME", tmp_path)
    (tmp_path / "tests/run/c").mkdir(parents=True)
    (tmp_path / "tests/run/c/a.c").write_text("int a;")
    bins, built = tmp_path / "bins", tmp_path / "built"
    for one in (bins, built):
        one.mkdir()
        (one / "llrm-cc").write_text('#!/bin/sh\nout=""\nwhile [ $# -gt 0 ]; do [ "$1" = -o ] && out=$2; shift; done\necho "${LLRM_NO_COMBINE:-on}" > "$out"\n')
        (one / "llrm-cc").chmod(0o755)
        (one / "llrm-llc").write_text("#!/bin/sh\n")
        (one / "llrm-llc").chmod(0o755)
    assert "equals" in postland.combine_premise(tmp_path, bins, built) and "a.c differs" in postland.combine_premise(tmp_path, bins, built)
    (built / "llrm-llc").write_text("#!/bin/sh\nexit 0\n")
    assert "DIFFERS FROM" in postland.combine_premise(tmp_path, bins, built)


def test_many_unit_failures_share_one_workspace_run_per_commit(monkeypatch):
    """56 failures x 6 probes of one workspace test run each was 22 hours of bisect; probes of unit failures now read one cached failing set per commit."""
    runs = []
    monkeypatch.setattr(postland, "tree", lambda sha: sha)
    monkeypatch.setattr(postland, "unit_failures", lambda at, name=None: (runs.append(at) or ({"unit a::t1", "unit a::t2"} if at >= "c" else set()), ""))
    postland.UNIT_SETS.clear()
    commits = list("abcdef")
    found = [postland.culprit(commits, lambda sha, one=one: postland.probe(one, sha)) for one in ("unit a::t1", "unit a::t2")]
    assert found == ["c", "c"] and len(runs) == len(set(runs)) <= 4


def test_every_function_check_calls_exists():
    """A slice edit dropped `smoke` and the run died after its tests/run, libs and 40 minutes with NameError."""
    import ast

    tree = ast.parse((Path(postland.__file__)).read_text())
    defined = {node.name for node in tree.body if isinstance(node, ast.FunctionDef)}
    called = {node.func.id for fn in tree.body if isinstance(fn, ast.FunctionDef) and fn.name in ("check", "once", "probe") for node in ast.walk(fn) if isinstance(node, ast.Call) and isinstance(node.func, ast.Name)}
    builtins_ = set(dir(__builtins__)) | {"set", "sorted", "len", "dict", "str", "int", "isinstance"}
    assert not {name for name in called - defined - builtins_ if name[0].islower() and name not in ("emit",)}, called - defined


def test_check_runs_smoke_once_and_names_its_result_in_the_covered_block(monkeypatch, tmp_path):
    """A slice edit dropped the smoke step; the run lost it silently. check() with stubbed phases must call smoke() once and report its line."""
    calls = []
    monkeypatch.setattr(postland, "tree", lambda sha: tmp_path)
    monkeypatch.setattr(postland, "dist", lambda at, sha: tmp_path)
    monkeypatch.setattr(postland, "combine_premise", lambda *args: "premise: ok")
    monkeypatch.setattr(postland, "run_groups", lambda at, bins, only=None: (set(), "57 groups, 863 programs, 0 failing", {}))
    monkeypatch.setattr(postland, "unit_failures", lambda at, name=None: (set(), "llrm-c 173/0"))
    monkeypatch.setattr(postland, "smoke", lambda at, bins, previous: (calls.append(previous) or {"smoke ratchet:x"}, "SMOKE LINE"))
    failing, covered = postland.check("f" * 40, "e" * 40)
    assert calls == ["e" * 40] and "smoke ratchet:x" in failing
    assert any("SMOKE LINE" in line for line in covered)


def test_a_culprit_naming_an_open_pr_is_rejected_and_a_merged_one_stands():
    """watcom_words was blamed on #2892, an open unmerged PR: its change is not on the head, so the bisect was wrong."""
    assert postland.verified_pr(2892, lambda pr: "OPEN") == (None, "#2892 is OPEN, not merged: rejected")
    assert postland.verified_pr(3026, lambda pr: "MERGED") == (3026, "")
    assert postland.verified_pr(None)[0] is None


def test_a_probe_commit_that_does_not_build_stops_the_bisect_instead_of_reading_as_passing(monkeypatch, tmp_path):
    monkeypatch.setattr(postland, "tree", lambda sha: tmp_path)
    monkeypatch.setattr(postland, "unit_failures", lambda at, name=None: ({postland.NAMELESS}, ""))
    postland.UNIT_SETS.clear()
    import pytest

    with pytest.raises(postland.Inconclusive):
        postland.probe("unit a::t", "c" * 40)


def test_a_smoke_failure_has_an_honest_repro_and_is_not_bisected_as_a_cargo_package():
    """The ratchet rows read `cargo test -p ratchet:facts_rewrite`, a command that cannot run."""
    assert "parity_smoke" in postland.repro("smoke ratchet:facts_rewrite") and "-p ratchet" not in postland.repro("smoke ratchet:facts_rewrite")
    import pytest

    with pytest.raises(postland.Inconclusive):
        postland.probe("smoke ratchet:x", "c" * 40)


def test_a_nameless_failure_is_retried_once_before_a_commit_is_called_unbuildable(monkeypatch, tmp_path):
    """The first workspace run at 2abaf530f failed nameless under load, its rerun named 62 failures; 55 bisects read 'does not build'."""
    answers = iter([({postland.NAMELESS}, ""), ({"unit a::t"}, "")])
    monkeypatch.setattr(postland, "tree", lambda sha: tmp_path)
    monkeypatch.setattr(postland, "unit_failures", lambda at, name=None: next(answers))
    postland.UNIT_SETS.clear()
    assert postland.probe("unit a::t", "c" * 40) is True


def test_a_nameless_run_leaves_its_output_tail_and_the_load_in_a_log(monkeypatch, tmp_path):
    """The retry hid why the first run failed; the tail of its output and the load average are kept."""
    monkeypatch.setattr(postland, "HOME", tmp_path)
    monkeypatch.setattr(postland, "tree", lambda sha: tmp_path)
    answers = iter([({postland.NAMELESS}, ""), (set(), "")])

    def fake(at, name=None):
        postland.LAST_OUTPUT["text"] = "error: killed by signal 9"
        return next(answers)

    monkeypatch.setattr(postland, "unit_failures", fake)
    postland.UNIT_SETS.clear()
    postland.probe("unit a::t", "d" * 40)
    log = (tmp_path / ("nameless-" + "d" * 9 + ".log")).read_text()
    assert "load" in log and "killed by signal 9" in log


def test_the_pr_of_a_commit_is_the_one_that_merged_it_not_the_first_that_contains_it():
    """Every culprit read '#2137 is OPEN, rejected': GitHub listed my branch's PR (which merged the head into itself) before the PR that landed the commit."""
    pulls = [{"number": 2137, "merge_commit_sha": "x"}, {"number": 3026, "merge_commit_sha": "abc"}]
    assert postland.merging_pr("abc", pulls) == 3026
    assert postland.merging_pr("zzz", pulls) is None
